//! A blocking mutex implementation.

use core::{
    cell::Cell,
    hint::spin_loop,
    sync::atomic::{AtomicU64, Ordering},
};

use ax_task::{WaitQueue, current};

static HANDOFF_ACCEPT_COUNT: AtomicU64 = AtomicU64::new(0);
static HANDOFF_FALLBACK_COUNT: AtomicU64 = AtomicU64::new(0);
static HANDOFF_DIRECT_UNLOCK_COUNT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct HandoffStats {
    pub accepts: u64,
    pub fallbacks: u64,
    pub direct_unlocks: u64,
}

impl HandoffStats {
    pub const fn total_unlocks(self) -> u64 {
        self.accepts + self.fallbacks + self.direct_unlocks
    }
}

pub fn handoff_stats() -> HandoffStats {
    HandoffStats {
        accepts: HANDOFF_ACCEPT_COUNT.load(Ordering::Relaxed),
        fallbacks: HANDOFF_FALLBACK_COUNT.load(Ordering::Relaxed),
        direct_unlocks: HANDOFF_DIRECT_UNLOCK_COUNT.load(Ordering::Relaxed),
    }
}

pub fn reset_handoff_stats() {
    HANDOFF_ACCEPT_COUNT.store(0, Ordering::Relaxed);
    HANDOFF_FALLBACK_COUNT.store(0, Ordering::Relaxed);
    HANDOFF_DIRECT_UNLOCK_COUNT.store(0, Ordering::Relaxed);
}

/// A [`lock_api::RawMutex`] implementation.
///
/// When the mutex is locked, the current task will block and be put into the
/// wait queue. When the mutex is unlocked, ownership is handed off to at most
/// one task waiting on the queue; if no tasks are waiting, the mutex simply
/// becomes unlocked.
pub struct RawMutex {
    wq: WaitQueue,
    state: AtomicU64,
}

impl RawMutex {
    const TAG_BITS: u64 = 2;
    const EPOCH_BITS: u64 = 16;
    const TASK_BITS: u64 = 64 - Self::TAG_BITS - Self::EPOCH_BITS;
    const TASK_MASK: u64 = (1_u64 << Self::TASK_BITS) - 1;
    const EPOCH_SHIFT: u64 = Self::TASK_BITS;
    const TAG_SHIFT: u64 = Self::TASK_BITS + Self::EPOCH_BITS;
    const EPOCH_MASK: u64 = (1_u64 << Self::EPOCH_BITS) - 1;

    const TAG_UNLOCKED: u64 = 0;
    const TAG_OWNED: u64 = 1;
    const TAG_HANDOFF_PENDING: u64 = 2;

    const HANDOFF_ACCEPT_SPINS: usize = 32;
    const HANDOFF_ACCEPT_YIELDS: usize = 4;

    /// Creates a [`RawMutex`].
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            wq: WaitQueue::new(),
            state: AtomicU64::new(Self::encode_unlocked(0)),
        }
    }

    #[inline(always)]
    const fn encode_state(tag: u64, epoch: u64, task_id: u64) -> u64 {
        debug_assert!(task_id <= Self::TASK_MASK);
        (tag << Self::TAG_SHIFT)
            | ((epoch & Self::EPOCH_MASK) << Self::EPOCH_SHIFT)
            | (task_id & Self::TASK_MASK)
    }

    #[inline(always)]
    const fn encode_unlocked(epoch: u64) -> u64 {
        Self::encode_state(Self::TAG_UNLOCKED, epoch, 0)
    }

    #[inline(always)]
    const fn encode_owned(owner_id: u64, epoch: u64) -> u64 {
        Self::encode_state(Self::TAG_OWNED, epoch, owner_id)
    }

    #[inline(always)]
    const fn encode_handoff(target_id: u64, epoch: u64) -> u64 {
        Self::encode_state(Self::TAG_HANDOFF_PENDING, epoch, target_id)
    }

    #[inline(always)]
    fn tag(state: u64) -> u64 {
        state >> Self::TAG_SHIFT
    }

    #[inline(always)]
    fn epoch(state: u64) -> u64 {
        (state >> Self::EPOCH_SHIFT) & Self::EPOCH_MASK
    }

    #[inline(always)]
    fn task_id(state: u64) -> u64 {
        state & Self::TASK_MASK
    }

    #[inline(always)]
    fn next_epoch(state: u64) -> u64 {
        Self::epoch(state).wrapping_add(1) & Self::EPOCH_MASK
    }

    #[inline(always)]
    fn is_unlocked(state: u64) -> bool {
        Self::tag(state) == Self::TAG_UNLOCKED
    }

    #[inline(always)]
    fn is_owned_by(state: u64, owner_id: u64) -> bool {
        Self::tag(state) == Self::TAG_OWNED && Self::task_id(state) == owner_id
    }

    #[inline(always)]
    fn is_handoff_to(state: u64, task_id: u64) -> bool {
        Self::tag(state) == Self::TAG_HANDOFF_PENDING && Self::task_id(state) == task_id
    }

    #[inline(always)]
    fn load_state(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    #[inline(always)]
    fn try_acquire(&self, current_id: u64, state: u64) -> bool {
        let (next, from_handoff) = match Self::tag(state) {
            Self::TAG_UNLOCKED => (Self::encode_owned(current_id, Self::epoch(state)), false),
            Self::TAG_HANDOFF_PENDING if Self::task_id(state) == current_id => {
                (Self::encode_owned(current_id, Self::epoch(state)), true)
            }
            _ => return false,
        };

        let acquired = self
            .state
            .compare_exchange_weak(state, next, Ordering::Acquire, Ordering::Relaxed)
            .is_ok();
        if acquired && from_handoff {
            HANDOFF_ACCEPT_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        acquired
    }

    #[inline(always)]
    fn validate_task_id(task_id: u64) {
        assert!(
            task_id <= Self::TASK_MASK,
            "Task ID {task_id} exceeds mutex state encoding capacity"
        );
    }
}

impl Default for RawMutex {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl lock_api::RawMutex for RawMutex {
    type GuardMarker = lock_api::GuardSend;

    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = RawMutex::new();

    #[inline(always)]
    fn lock(&self) {
        let current_id = current().id().as_u64();
        Self::validate_task_id(current_id);

        loop {
            let state = self.load_state();
            assert!(
                !Self::is_owned_by(state, current_id),
                "Thread({current_id}) tried to acquire mutex it already owns.",
            );

            if self.try_acquire(current_id, state) {
                break;
            }

            self.wq.wait_until(|| {
                let state = self.load_state();
                Self::is_unlocked(state) || Self::is_handoff_to(state, current_id)
            });
        }
    }

    #[inline(always)]
    fn try_lock(&self) -> bool {
        let current_id = current().id().as_u64();
        Self::validate_task_id(current_id);

        let state = self.load_state();
        assert!(
            !Self::is_owned_by(state, current_id),
            "Thread({current_id}) tried to acquire mutex it already owns.",
        );
        match Self::tag(state) {
            Self::TAG_UNLOCKED => self
                .state
                .compare_exchange(
                    state,
                    Self::encode_owned(current_id, Self::epoch(state)),
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok(),
            Self::TAG_HANDOFF_PENDING if Self::task_id(state) == current_id => self
                .state
                .compare_exchange(
                    state,
                    Self::encode_owned(current_id, Self::epoch(state)),
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok(),
            _ => false,
        }
    }

    #[inline(always)]
    unsafe fn unlock(&self) {
        let current_id = current().id().as_u64();
        let state = self.load_state();
        assert_eq!(
            Self::tag(state),
            Self::TAG_OWNED,
            "Thread({current_id}) tried to release mutex it doesn't own",
        );
        assert_eq!(
            Self::task_id(state),
            current_id,
            "Thread({current_id}) tried to release mutex it doesn't own",
        );

        let next_epoch = Self::next_epoch(state);
        let unlocked = Self::encode_unlocked(next_epoch);

        let handoff_state = Cell::new(0);
        let handed_off = self.wq.notify_one_with(true, |task_id: u64| {
            if task_id == 0 {
                HANDOFF_DIRECT_UNLOCK_COUNT.fetch_add(1, Ordering::Relaxed);
                let _ = self.state.compare_exchange(
                    state,
                    unlocked,
                    Ordering::Release,
                    Ordering::Relaxed,
                );
                return;
            }

            Self::validate_task_id(task_id);
            let pending = Self::encode_handoff(task_id, next_epoch);
            handoff_state.set(pending);
            let _ =
                self.state
                    .compare_exchange(state, pending, Ordering::Release, Ordering::Relaxed);
        });

        if !handed_off {
            return;
        }

        let handoff_state = handoff_state.get();

        for _ in 0..Self::HANDOFF_ACCEPT_SPINS {
            if self.load_state() != handoff_state {
                return;
            }
            spin_loop();
        }

        for _ in 0..Self::HANDOFF_ACCEPT_YIELDS {
            ax_task::yield_now();
            if self.load_state() != handoff_state {
                return;
            }
        }

        if self
            .state
            .compare_exchange(
                handoff_state,
                unlocked,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            HANDOFF_FALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
            self.wq.notify_one(true);
        }
    }

    #[inline(always)]
    fn is_locked(&self) -> bool {
        !Self::is_unlocked(self.load_state())
    }
}

pub type Mutex<T> = lock_api::Mutex<RawMutex, T>;
pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, RawMutex, T>;

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use ax_task as thread;

    use crate::Mutex;

    static INIT: Once = Once::new();

    fn may_interrupt() {
        if fastrand::u8(0..3) == 0 {
            thread::yield_now();
        }
    }

    #[test]
    fn lots_and_lots() {
        INIT.call_once(thread::init_scheduler);

        const NUM_TASKS: u32 = 10;
        const NUM_ITERS: u32 = 10_000;
        static M: Mutex<u32> = Mutex::new(0);

        fn inc(delta: u32) {
            for _ in 0..NUM_ITERS {
                let mut val = M.lock();
                *val += delta;
                may_interrupt();
                drop(val);
                may_interrupt();
            }
        }

        for _ in 0..NUM_TASKS {
            thread::spawn(|| inc(1));
            thread::spawn(|| inc(2));
        }

        println!("spawn OK");
        loop {
            let val = M.lock();
            if *val == NUM_ITERS * NUM_TASKS * 3 {
                break;
            }
            may_interrupt();
            drop(val);
            may_interrupt();
        }

        assert_eq!(*M.lock(), NUM_ITERS * NUM_TASKS * 3);
        println!("Mutex test OK");
    }
}
