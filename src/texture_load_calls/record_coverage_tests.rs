use super::*;

#[test]
fn image_loads_reject_palette_and_empty_records() {
    assert!(file_record_matches_operation(1, 64, 32, 2_048));
    assert!(!file_record_matches_operation(1, 0, 0, 1_024));
    assert!(!file_record_matches_operation(1, 0, 0, 0));
}

#[test]
fn palette_loads_require_exact_rgba32_palette_records() {
    assert!(file_record_matches_operation(2, 0, 0, 256 * 4));
    assert!(!file_record_matches_operation(2, 0, 0, 255 * 4));
    assert!(!file_record_matches_operation(2, 16, 16, 256 * 4));
}

#[test]
fn auxiliary_loads_accept_any_in_range_record_role() {
    assert!(file_record_matches_operation(0, 0, 0, 0));
    assert!(file_record_matches_operation(0, 64, 32, 2_048));
}
