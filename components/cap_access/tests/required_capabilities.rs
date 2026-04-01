use cap_access::{Cap, WithCap};

#[test]
fn requires_all_requested_capabilities() {
    let full = WithCap::new(42, Cap::READ | Cap::WRITE);
    assert!(full.can_access(Cap::READ | Cap::WRITE));
    assert_eq!(full.access(Cap::READ | Cap::WRITE), Some(&42));

    let read_only = WithCap::new(7, Cap::READ);
    assert!(!read_only.can_access(Cap::READ | Cap::WRITE));
    assert_eq!(read_only.access(Cap::READ | Cap::WRITE), None);
}

#[test]
fn access_or_err_rejects_missing_bits() {
    let exec_only = WithCap::new("kernel", Cap::EXECUTE);
    assert_eq!(
        exec_only.access_or_err(Cap::READ | Cap::EXECUTE, "missing required capability"),
        Err("missing required capability")
    );
}
