use cap_access::{Cap, WithCap};

#[test]
fn empty_capability_request_is_always_allowed() {
    let read_only = WithCap::new(11, Cap::READ);
    assert!(read_only.can_access(Cap::empty()));
    assert_eq!(read_only.access(Cap::empty()), Some(&11));
}

#[test]
fn combo_request_requires_all_bits_not_any_one_bit() {
    let exec_only = WithCap::new("vm", Cap::EXECUTE);
    assert!(!exec_only.can_access(Cap::READ | Cap::EXECUTE));
    assert_eq!(exec_only.access(Cap::READ | Cap::EXECUTE), None);
}
