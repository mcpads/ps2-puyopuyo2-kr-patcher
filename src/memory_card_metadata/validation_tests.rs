use super::*;

#[test]
fn dispatch_table_preserves_physical_entry_order() {
    let source = [0x10, 0, 0x40, 0, 0x20, 0, 0x40, 0, 0x30, 0, 0x40, 0];

    let entries = read_pointer_table(&source, 3).unwrap();

    assert_eq!(entries, [0x0040_0010, 0x0040_0020, 0x0040_0030]);
}

#[test]
fn dispatch_table_rejects_a_changed_population() {
    let error = read_pointer_table(&[0; 8], 3).unwrap_err();

    assert!(error.to_string().contains("dispatch table length changed"));
}
