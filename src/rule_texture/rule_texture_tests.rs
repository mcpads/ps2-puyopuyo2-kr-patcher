use super::*;

#[test]
fn selection_patterns_preserve_background_and_option_sheets() {
    let records = EXPECTED_SELECT_PATTERN_INDICES
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
        SELECT_PATTERN_SYMBOL,
        &records,
        &EXPECTED_SELECT_PATTERN_INDICES,
    )
    .unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|record| record.image_load_index == 0)
            .count(),
        2
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| record.image_load_index == 1)
            .count(),
        14
    );
}

#[test]
fn load_records_reject_a_changed_palette_file() {
    let mut records = EXPECTED_LOAD_RECORDS
        .into_iter()
        .map(|(flags, operation, file_record, destination)| LoadRecord {
            flags,
            operation,
            file_record,
            destination,
        })
        .collect::<Vec<_>>();
    records[5].file_record += 1;
    let error = validate_rule_load_records(&records)
        .unwrap_err()
        .to_string();
    assert!(error.contains("load records changed"));
}
