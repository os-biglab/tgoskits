#![cfg_attr(feature = "ax-std", no_std)]
#![cfg_attr(feature = "ax-std", no_main)]

#[macro_use]
#[cfg(feature = "ax-std")]
extern crate ax_std as std;

#[cfg(feature = "ax-std")]
use std::os::arceos::api::task::{
    self as api, AxCpuMask, AxWaitQueueHandle, ax_set_current_affinity,
};
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

#[cfg(feature = "ax-std")]
use ax_sync::{handoff_stats, reset_handoff_stats};

const WORKERS_PER_CPU: usize = 6;
const ROUNDS: usize = 160;
const WAIT_SLICE_MS: u64 = 2;
const WATCHDOG_SECS: u64 = 12;
const ROUND_LOG_INTERVAL: usize = 10;
const ENABLE_MIGRATION: bool = true;
const YIELD_IN_CRITICAL_SECTION: bool = true;
const PIN_ALL_WORKERS_TO_CPU0: bool = false;
const SAMPLE_WORKERS: usize = 8;
const SAMPLE_ROUNDS: usize = 12;

static ROUND_BARRIER: AxWaitQueueHandle = AxWaitQueueHandle::new();
static RELEASE_WQ: AxWaitQueueHandle = AxWaitQueueHandle::new();
static START_WQ: AxWaitQueueHandle = AxWaitQueueHandle::new();

static STARTED: AtomicUsize = AtomicUsize::new(0);
static ARRIVED: AtomicUsize = AtomicUsize::new(0);
static RELEASED_ROUND: AtomicUsize = AtomicUsize::new(0);
static COMPLETED: AtomicUsize = AtomicUsize::new(0);
static PROGRESS: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicUsize = AtomicUsize::new(0);
static WATCHDOG_STOP: AtomicBool = AtomicBool::new(false);

static SHARED_TOTAL: Mutex<u64> = Mutex::new(0);
const MAX_WORKERS: usize = 64;
static WORKER_STATE: [AtomicU8; MAX_WORKERS] = [const { AtomicU8::new(0) }; MAX_WORKERS];
static WORKER_ROUND: [AtomicUsize; MAX_WORKERS] = [const { AtomicUsize::new(0) }; MAX_WORKERS];
static WORKER_TASK_ID: [AtomicU64; MAX_WORKERS] = [const { AtomicU64::new(0) }; MAX_WORKERS];

const STAGE_WAIT_START: u8 = 0;
const STAGE_BEFORE_RELEASE: u8 = 1;
const STAGE_AFTER_RELEASE: u8 = 2;
const STAGE_BEFORE_LOCK: u8 = 3;
const STAGE_IN_MUTEX: u8 = 4;
const STAGE_AFTER_MUTEX: u8 = 5;
const STAGE_DONE: u8 = 6;

#[cfg(feature = "ax-std")]
fn online_cpu_mask() -> AxCpuMask {
    let cpu_num = thread::available_parallelism().unwrap().get();
    let mut cpumask = AxCpuMask::new();
    for cpu_id in 0..cpu_num {
        cpumask.set(cpu_id, true);
    }
    cpumask
}

#[cfg(feature = "ax-std")]
fn pin_worker(worker_id: usize, round: usize, cpu_num: usize) {
    if PIN_ALL_WORKERS_TO_CPU0 {
        let _ = (worker_id, round, cpu_num);
        ax_set_current_affinity(AxCpuMask::one_shot(0))
            .unwrap_or_else(|err| panic!("failed to pin worker to cpu0: {err:?}"));
        return;
    }
    if !ENABLE_MIGRATION {
        return;
    }
    if cpu_num <= 1 {
        return;
    }
    let target = (worker_id + round) % cpu_num;
    ax_set_current_affinity(AxCpuMask::one_shot(target))
        .unwrap_or_else(|err| panic!("failed to set affinity for worker {worker_id}: {err:?}"));
}

fn wait_for_release(expected_round: usize) {
    loop {
        if RELEASED_ROUND.load(Ordering::Acquire) >= expected_round {
            return;
        }
        let timed_out = api::ax_wait_queue_wait_until(
            &RELEASE_WQ,
            || RELEASED_ROUND.load(Ordering::Acquire) >= expected_round,
            Some(Duration::from_millis(WAIT_SLICE_MS)),
        );
        if timed_out {
            TIMEOUTS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn set_worker_stage(worker_id: usize, round: usize, stage: u8) {
    WORKER_ROUND[worker_id].store(round, Ordering::Release);
    WORKER_STATE[worker_id].store(stage, Ordering::Release);
}

fn stage_name(stage: u8) -> &'static str {
    match stage {
        STAGE_WAIT_START => "wait_start",
        STAGE_BEFORE_RELEASE => "before_release",
        STAGE_AFTER_RELEASE => "after_release",
        STAGE_BEFORE_LOCK => "before_lock",
        STAGE_IN_MUTEX => "in_mutex",
        STAGE_AFTER_MUTEX => "after_mutex",
        STAGE_DONE => "done",
        _ => "unknown",
    }
}

fn should_log_worker_round(worker_id: usize, round: usize) -> bool {
    worker_id < SAMPLE_WORKERS && round < SAMPLE_ROUNDS
}

fn worker_entry(worker_id: usize, cpu_num: usize, total_workers: usize) {
    #[cfg(feature = "ax-std")]
    {
        WORKER_TASK_ID[worker_id].store(api::ax_current_task_id(), Ordering::Release);
    }
    STARTED.fetch_add(1, Ordering::Release);
    api::ax_wait_queue_wake(&START_WQ, 1);
    api::ax_wait_queue_wait_until(
        &START_WQ,
        || STARTED.load(Ordering::Acquire) == total_workers,
        None,
    );
    api::ax_wait_queue_wake(&START_WQ, u32::MAX);

    #[cfg(feature = "ax-std")]
    if PIN_ALL_WORKERS_TO_CPU0 {
        ax_set_current_affinity(AxCpuMask::one_shot(0))
            .unwrap_or_else(|err| panic!("failed to pin worker to cpu0: {err:?}"));
    }

    for round in 0..ROUNDS {
        set_worker_stage(worker_id, round, STAGE_WAIT_START);
        #[cfg(feature = "ax-std")]
        pin_worker(worker_id, round, cpu_num);

        ARRIVED.fetch_add(1, Ordering::AcqRel);
        api::ax_wait_queue_wake(&ROUND_BARRIER, 1);

        set_worker_stage(worker_id, round, STAGE_BEFORE_RELEASE);
        wait_for_release(round + 1);

        set_worker_stage(worker_id, round, STAGE_AFTER_RELEASE);
        set_worker_stage(worker_id, round, STAGE_BEFORE_LOCK);
        let mut guard = SHARED_TOTAL.lock();
        if should_log_worker_round(worker_id, round) {
            println!(
                "worker_entered_cs: worker={worker_id}, task_id={}, round={round}",
                WORKER_TASK_ID[worker_id].load(Ordering::Acquire),
            );
        }
        set_worker_stage(worker_id, round, STAGE_IN_MUTEX);
        *guard += 1;
        if YIELD_IN_CRITICAL_SECTION {
            thread::yield_now();
        }
        drop(guard);

        set_worker_stage(worker_id, round, STAGE_AFTER_MUTEX);
        if should_log_worker_round(worker_id, round) {
            println!(
                "worker_after_unlock: worker={worker_id}, task_id={}, round={round}",
                WORKER_TASK_ID[worker_id].load(Ordering::Acquire),
            );
        }
        let _ = worker_id;
        PROGRESS.fetch_add(1, Ordering::Release);
    }

    set_worker_stage(worker_id, ROUNDS, STAGE_DONE);
    COMPLETED.fetch_add(1, Ordering::Release);
    api::ax_wait_queue_wake(&ROUND_BARRIER, 1);
}

fn coordinator_entry(total_workers: usize) {
    for round in 0..ROUNDS {
        let expected = (round + 1) * total_workers;
        api::ax_wait_queue_wait_until(
            &ROUND_BARRIER,
            || ARRIVED.load(Ordering::Acquire) >= expected,
            None,
        );

        if round % 4 == 0 {
            thread::sleep(Duration::from_millis(WAIT_SLICE_MS));
        } else {
            thread::yield_now();
        }

        RELEASED_ROUND.store(round + 1, Ordering::Release);
        api::ax_wait_queue_wake(&RELEASE_WQ, u32::MAX);

        if round % ROUND_LOG_INTERVAL == 0 || round + 1 == ROUNDS {
            println!(
                "coordinator_release: round={}/{ROUNDS}, expected_arrived={}, actual_arrived={}, \
                 progress={}",
                round + 1,
                expected,
                ARRIVED.load(Ordering::Acquire),
                PROGRESS.load(Ordering::Acquire),
            );
        }
    }
}

fn watchdog_entry(expected_progress: u64, total_workers: usize) {
    let mut stagnant_ticks = 0;
    let mut last_progress = PROGRESS.load(Ordering::Acquire);
    let mut last_timeouts = TIMEOUTS.load(Ordering::Acquire);

    while !WATCHDOG_STOP.load(Ordering::Acquire) {
        thread::sleep(Duration::from_secs(1));
        let current_progress = PROGRESS.load(Ordering::Acquire);
        if current_progress >= expected_progress {
            return;
        }
        let current_timeouts = TIMEOUTS.load(Ordering::Acquire);
        let timeout_delta = current_timeouts.saturating_sub(last_timeouts);
        last_timeouts = current_timeouts;
        if current_progress == last_progress {
            stagnant_ticks += 1;
        } else {
            stagnant_ticks = 0;
            last_progress = current_progress;
        }

        println!(
            "watchdog: progress={current_progress}/{expected_progress}, \
             timeouts={current_timeouts}, timeout_delta={timeout_delta}, \
             stagnant_ticks={stagnant_ticks}, arrived={}, released={}, completed={}",
            ARRIVED.load(Ordering::Relaxed),
            RELEASED_ROUND.load(Ordering::Relaxed),
            COMPLETED.load(Ordering::Relaxed),
        );

        if stagnant_ticks >= 1 {
            let mut stage_counts = [0usize; 7];
            let mut sample = std::vec::Vec::new();
            for worker_id in 0..total_workers {
                let stage = WORKER_STATE[worker_id].load(Ordering::Acquire);
                if let Some(count) = stage_counts.get_mut(stage as usize) {
                    *count += 1;
                }
                if sample.len() < 8 && stage != STAGE_DONE {
                    sample.push((
                        worker_id,
                        WORKER_TASK_ID[worker_id].load(Ordering::Acquire),
                        stage,
                        WORKER_ROUND[worker_id].load(Ordering::Acquire),
                    ));
                }
            }
            println!(
                "watchdog_summary: wait_start={}, before_release={}, after_release={}, \
                 before_lock={}, in_mutex={}, after_mutex={}, done={}",
                stage_counts[STAGE_WAIT_START as usize],
                stage_counts[STAGE_BEFORE_RELEASE as usize],
                stage_counts[STAGE_AFTER_RELEASE as usize],
                stage_counts[STAGE_BEFORE_LOCK as usize],
                stage_counts[STAGE_IN_MUTEX as usize],
                stage_counts[STAGE_AFTER_MUTEX as usize],
                stage_counts[STAGE_DONE as usize],
            );
            for (worker_id, task_id, stage, round) in sample {
                println!(
                    "watchdog_sample: worker={worker_id}, task_id={task_id}, stage={}, \
                     round={round}",
                    stage_name(stage),
                );
            }
        }

        assert!(
            stagnant_ticks < WATCHDOG_SECS,
            "watchdog: no forward progress for {WATCHDOG_SECS}s, \
             progress={current_progress}/{expected_progress}, arrived={}, released={}, \
             completed={}",
            ARRIVED.load(Ordering::Relaxed),
            RELEASED_ROUND.load(Ordering::Relaxed),
            COMPLETED.load(Ordering::Relaxed),
        );
    }
}

#[cfg_attr(feature = "ax-std", unsafe(no_mangle))]
fn main() {
    let cpu_num = thread::available_parallelism().unwrap().get();
    let total_workers = cpu_num * WORKERS_PER_CPU;
    let expected_progress = (total_workers * ROUNDS) as u64;

    println!(
        "concurrency_stress: cpu_num={cpu_num}, total_workers={total_workers}, rounds={ROUNDS}, \
         migration={ENABLE_MIGRATION}, yield_in_cs={YIELD_IN_CRITICAL_SECTION}"
    );
    #[cfg(feature = "ax-std")]
    reset_handoff_stats();
    assert!(
        total_workers <= MAX_WORKERS,
        "increase MAX_WORKERS for this test"
    );
    for worker_id in 0..total_workers {
        WORKER_STATE[worker_id].store(STAGE_WAIT_START, Ordering::Release);
        WORKER_ROUND[worker_id].store(0, Ordering::Release);
        WORKER_TASK_ID[worker_id].store(0, Ordering::Release);
    }
    #[cfg(feature = "ax-std")]
    ax_set_current_affinity(online_cpu_mask()).expect("failed to set main task affinity");

    let watchdog = thread::spawn(move || watchdog_entry(expected_progress, total_workers));
    let coordinator = thread::spawn(move || coordinator_entry(total_workers));

    let mut workers = std::vec::Vec::with_capacity(total_workers);
    for worker_id in 0..total_workers {
        workers.push(thread::spawn(move || {
            worker_entry(worker_id, cpu_num, total_workers)
        }));
    }

    for worker in workers {
        worker.join().unwrap();
    }
    coordinator.join().unwrap();

    WATCHDOG_STOP.store(true, Ordering::Release);
    watchdog.join().unwrap();

    let actual_total = *SHARED_TOTAL.lock();
    let expected_total = expected_progress;
    #[cfg(feature = "ax-std")]
    {
        let stats = handoff_stats();
        let handoff_total = stats.accepts + stats.fallbacks;
        let accept_pct = (stats.accepts * 100)
            .checked_div(handoff_total)
            .unwrap_or(0);
        println!(
            "mutex_handoff_stats: accepts={}, fallbacks={}, direct_unlocks={}, handoff_total={}, \
             accept_pct={}%, total_unlocks={}",
            stats.accepts,
            stats.fallbacks,
            stats.direct_unlocks,
            handoff_total,
            accept_pct,
            stats.total_unlocks(),
        );
    }
    println!(
        "concurrency_stress: progress={}, timeouts={}, completed={}",
        PROGRESS.load(Ordering::Acquire),
        TIMEOUTS.load(Ordering::Acquire),
        COMPLETED.load(Ordering::Acquire),
    );
    assert_eq!(actual_total, expected_total);
    assert_eq!(COMPLETED.load(Ordering::Acquire), total_workers);

    println!("All tests passed!");
}
