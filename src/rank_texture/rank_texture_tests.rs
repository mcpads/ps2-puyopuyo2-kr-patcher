use super::*;
use crate::indexed_texture::FileRecord;

#[test]
fn load_records_preserve_files_flags_and_gs_destinations() {
    let records = EXPECTED_LOAD_RECORDS
        .into_iter()
        .map(|(flags, operation, file_record, destination)| LoadRecord {
            flags,
            operation,
            file_record,
            destination,
        })
        .collect::<Vec<_>>();
    assert_eq!(validate_rank_load_records(&records).unwrap(), 6);
}

#[test]
fn load_records_reject_a_changed_palette_flag() {
    let mut records = EXPECTED_LOAD_RECORDS
        .into_iter()
        .map(|(flags, operation, file_record, destination)| LoadRecord {
            flags,
            operation,
            file_record,
            destination,
        })
        .collect::<Vec<_>>();
    records[6].flags = 2;
    let error = validate_rank_load_records(&records)
        .unwrap_err()
        .to_string();
    assert!(error.contains("load records changed"));
}

#[test]
fn registration_preserves_the_reused_texture_pair() {
    let pairs = EXPECTED_PAIRS
        .into_iter()
        .map(|(image_load_index, palette_slot)| TextureRegistrationPair {
            image_load_index,
            palette_slot,
        })
        .collect::<Vec<_>>();
    let unique = validate_rank_pairs(&pairs).unwrap();
    assert_eq!(pairs.len(), 7);
    assert_eq!(unique.len(), 6);
    assert_eq!(
        pairs
            .iter()
            .filter(|pair| pair.image_load_index == 2 && pair.palette_slot == 2)
            .count(),
        2
    );
}

#[test]
fn registration_rejects_a_reordered_consumer_pair() {
    let mut pairs = EXPECTED_PAIRS
        .into_iter()
        .map(|(image_load_index, palette_slot)| TextureRegistrationPair {
            image_load_index,
            palette_slot,
        })
        .collect::<Vec<_>>();
    pairs.swap(0, 1);
    let error = validate_rank_pairs(&pairs).unwrap_err().to_string();
    assert!(error.contains("registration pairs changed"));
}

#[test]
fn patterns_resolve_registration_slots_before_validating_source_rectangles() {
    let pairs = EXPECTED_PAIRS
        .into_iter()
        .map(|(image_load_index, palette_slot)| TextureRegistrationPair {
            image_load_index,
            palette_slot,
        })
        .collect::<Vec<_>>();
    let loads = EXPECTED_LOAD_RECORDS
        .into_iter()
        .map(|(flags, operation, file_record, destination)| LoadRecord {
            flags,
            operation,
            file_record,
            destination,
        })
        .collect::<Vec<_>>();
    let mut files = vec![
        FileRecord {
            sector_offset: 0,
            sector_count: 1,
            stored_byte_length: 1,
            width: 1,
            height: 1,
        };
        243
    ];
    files[120].width = 640;
    files[120].height = 512;
    let pattern = TexturePatternRecord {
        image_load_index: 4,
        source_x: 256,
        source_y: 448,
        width: 256,
        height: 64,
    };

    validate_rank_patterns(&[pattern], &pairs, &loads, &files).unwrap();
    let resolved = RankPatternRecord::from_source(4, pattern, pairs[4]);
    assert_eq!(resolved.registration_slot, 4);
    assert_eq!(resolved.image_load_index, 1);
    assert_eq!(resolved.palette_slot, 1);
}
