use super::*;

#[test]
fn patterns_cover_every_loaded_image_in_consumer_order() {
    let records = EXPECTED_PATTERN_INDICES
        .into_iter()
        .map(|image_load_index| TexturePatternRecord {
            image_load_index,
            source_x: 0,
            source_y: 0,
            width: 1,
            height: 1,
        })
        .collect::<Vec<_>>();
    validate_pattern_indices(PATTERN_TABLE_SYMBOL, &records, &EXPECTED_PATTERN_INDICES).unwrap();
    for image_load_index in 0..3 {
        assert!(
            records
                .iter()
                .any(|record| record.image_load_index == image_load_index)
        );
    }
}

#[test]
fn load_records_reject_a_changed_image_destination() {
    let mut records = EXPECTED_LOAD_RECORDS
        .into_iter()
        .map(|(flags, operation, file_record, destination)| LoadRecord {
            flags,
            operation,
            file_record,
            destination,
        })
        .collect::<Vec<_>>();
    records[0].destination += 1;
    let error = validate_omake_load_records(&records)
        .unwrap_err()
        .to_string();
    assert!(error.contains("load records changed"));
}
