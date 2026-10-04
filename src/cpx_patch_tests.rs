use super::*;

#[test]
fn stored_length_patch_changes_only_the_selected_file_record_field() {
    let file_list_offset = 12;
    let record_index = 2;
    let field_offset =
        file_list_offset + record_index * IMAGE_FILE_RECORD_BYTES + STORED_LENGTH_FIELD_OFFSET;
    let mut executable = vec![0x55; 96];
    executable[field_offset..field_offset + 4].copy_from_slice(&40_u32.to_le_bytes());

    let (patched, writes) = patch_file_record_stored_length(
        &executable,
        FileRecordStoredLengthPatch {
            file_list_offset,
            record_index,
            expected_stored_length: 40,
            replacement_cpx_length: 31,
            region_id: "test-region",
            region_reason: "test record",
            write_id: "test-write",
            owner: "test-owner",
            purpose: "test purpose",
        },
    )
    .unwrap();

    assert_eq!(writes, 1);
    assert_eq!(
        &patched[field_offset..field_offset + 4],
        &30_u32.to_le_bytes()
    );
    assert_eq!(&patched[..field_offset], &executable[..field_offset]);
    assert_eq!(
        &patched[field_offset + 4..],
        &executable[field_offset + 4..]
    );
}

#[test]
fn stored_length_batch_updates_each_selected_record_and_preserves_other_bytes() {
    let file_list_offset = 8;
    let mut executable = vec![0x55; 96];
    let first_offset = file_list_offset + STORED_LENGTH_FIELD_OFFSET;
    let second_offset = file_list_offset + IMAGE_FILE_RECORD_BYTES + STORED_LENGTH_FIELD_OFFSET;
    executable[first_offset..first_offset + 4].copy_from_slice(&40_u32.to_le_bytes());
    executable[second_offset..second_offset + 4].copy_from_slice(&60_u32.to_le_bytes());
    let writes = [
        FileRecordStoredLengthWrite {
            file_list_offset,
            record_index: 0,
            expected_stored_length: 40,
            replacement_cpx_length: 31,
            region_id: "first-region".to_string(),
            region_reason: "first record".to_string(),
            write_id: "first-write".to_string(),
            owner: "test-owner".to_string(),
            purpose: "test first length".to_string(),
        },
        FileRecordStoredLengthWrite {
            file_list_offset,
            record_index: 1,
            expected_stored_length: 60,
            replacement_cpx_length: 51,
            region_id: "second-region".to_string(),
            region_reason: "second record".to_string(),
            write_id: "second-write".to_string(),
            owner: "test-owner".to_string(),
            purpose: "test second length".to_string(),
        },
    ];

    let (patched, write_count) = patch_file_record_stored_lengths(&executable, &writes).unwrap();

    assert_eq!(write_count, 2);
    assert_eq!(
        &patched[first_offset..first_offset + 4],
        &30_u32.to_le_bytes()
    );
    assert_eq!(
        &patched[second_offset..second_offset + 4],
        &50_u32.to_le_bytes()
    );
    assert_eq!(&patched[..first_offset], &executable[..first_offset]);
    assert_eq!(
        &patched[first_offset + 4..second_offset],
        &executable[first_offset + 4..second_offset]
    );
    assert_eq!(
        &patched[second_offset + 4..],
        &executable[second_offset + 4..]
    );
}
