use super::*;

#[test]
fn screen_patterns_preserve_both_consumer_texture_indices() {
    let records = EXPECTED_SCREEN_PATTERN_INDICES
        .into_iter()
        .map(|image_load_index| TexturePatternRecord {
            image_load_index,
            source_x: 0,
            source_y: 0,
            width: 1,
            height: 1,
        })
        .collect::<Vec<_>>();
    validate_pattern_indices(
        SCREEN_PATTERN_SYMBOL,
        &records,
        &EXPECTED_SCREEN_PATTERN_INDICES,
    )
    .unwrap();
    assert!(records.iter().any(|record| record.image_load_index == 0));
    assert!(records.iter().any(|record| record.image_load_index == 1));
}

#[test]
fn load_records_reject_a_changed_file_record() {
    let mut records = EXPECTED_LOAD_RECORDS
        .into_iter()
        .map(|(flags, operation, file_record, destination)| LoadRecord {
            flags,
            operation,
            file_record,
            destination,
        })
        .collect::<Vec<_>>();
    records[2].file_record += 1;
    let error = validate_continue_load_records(&records)
        .unwrap_err()
        .to_string();
    assert!(error.contains("load records changed"));
}
