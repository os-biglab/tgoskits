use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use timer_list::{TimeValue, TimerEvent, TimerList};

struct RecordEvent {
    id: usize,
    records: Arc<Mutex<Vec<(usize, TimeValue)>>>,
}

impl TimerEvent for RecordEvent {
    fn callback(self, now: TimeValue) {
        self.records.lock().unwrap().push((self.id, now));
    }
}

#[test]
fn shared_deadline_events_all_expire_on_exact_boundary() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let mut timers = TimerList::new();
    for id in 0..3 {
        timers.set(
            Duration::from_millis(3),
            RecordEvent {
                id,
                records: records.clone(),
            },
        );
    }

    let mut seen = Vec::new();
    while let Some((deadline, event)) = timers.expire_one(Duration::from_millis(3)) {
        event.callback(deadline);
        seen.push(deadline);
    }

    assert_eq!(seen, vec![Duration::from_millis(3); 3]);
    assert_eq!(records.lock().unwrap().len(), 3);
}

#[test]
fn next_deadline_advances_after_exact_expiration() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let mut timers = TimerList::new();
    timers.set(
        Duration::from_millis(1),
        RecordEvent {
            id: 1,
            records: records.clone(),
        },
    );
    timers.set(
        Duration::from_millis(4),
        RecordEvent {
            id: 4,
            records,
        },
    );

    let (deadline, event) = timers
        .expire_one(Duration::from_millis(1))
        .expect("exact-deadline event should be returned");
    event.callback(deadline);
    assert_eq!(deadline, Duration::from_millis(1));
    assert_eq!(timers.next_deadline(), Some(Duration::from_millis(4)));
}
