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
fn expires_event_at_exact_deadline() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let mut timers = TimerList::new();
    timers.set(
        Duration::from_millis(5),
        RecordEvent {
            id: 1,
            records: records.clone(),
        },
    );

    assert!(timers.expire_one(Duration::from_millis(4)).is_none());

    let (deadline, event) = timers
        .expire_one(Duration::from_millis(5))
        .expect("event should expire exactly at its deadline");
    assert_eq!(deadline, Duration::from_millis(5));
    event.callback(Duration::from_millis(5));

    assert_eq!(
        *records.lock().unwrap(),
        vec![(1, Duration::from_millis(5))]
    );
    assert!(timers.is_empty());
}

#[test]
fn zero_deadline_expires_immediately() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let mut timers = TimerList::new();
    timers.set(
        Duration::ZERO,
        RecordEvent {
            id: 7,
            records: records.clone(),
        },
    );

    let (deadline, event) = timers
        .expire_one(Duration::ZERO)
        .expect("zero deadline event should expire immediately");
    assert_eq!(deadline, Duration::ZERO);
    event.callback(Duration::ZERO);

    assert_eq!(*records.lock().unwrap(), vec![(7, Duration::ZERO)]);
}
