use super::*;

#[test]
fn changed_ranges_cover_only_actual_differences() {
    assert_eq!(changed_ranges(b"abcdefghi", b"abXXefYYi"), vec![2..4, 6..8]);
}

// Required behavior: independently built bundles may be composed only when each changed source
// byte is disjoint or has the same replacement; different replacements for one source byte fail.
#[test]
fn component_user_data_composes_disjoint_and_identical_changes() {
    let source = [0_u8; 8];
    let mut merged = source;
    let mut owner_by_offset = [None; 8];
    let owners = vec!["first".to_string(), "second".to_string()];
    let first = [0, 1, 0, 0, 0, 0, 7, 0];
    let second = [0, 0, 2, 0, 0, 0, 7, 0];

    let first_report = merge_component_user_data(
        &source,
        &mut merged,
        &mut owner_by_offset,
        &first,
        0,
        &owners,
        11,
    )
    .unwrap();
    let second_report = merge_component_user_data(
        &source,
        &mut merged,
        &mut owner_by_offset,
        &second,
        1,
        &owners,
        11,
    )
    .unwrap();

    assert_eq!(merged, [0, 1, 2, 0, 0, 0, 7, 0]);
    assert_eq!(first_report.added_user_data_bytes, 2);
    assert_eq!(second_report.added_user_data_bytes, 1);
    assert_eq!(second_report.repeated_identical_user_data_bytes, 1);
}

#[test]
fn component_user_data_rejects_conflicting_replacements() {
    let source = [0_u8; 4];
    let mut merged = source;
    let mut owner_by_offset = [None; 4];
    let owners = vec!["first".to_string(), "second".to_string()];
    merge_component_user_data(
        &source,
        &mut merged,
        &mut owner_by_offset,
        &[0, 1, 0, 0],
        0,
        &owners,
        29,
    )
    .unwrap();

    let error = merge_component_user_data(
        &source,
        &mut merged,
        &mut owner_by_offset,
        &[0, 2, 0, 0],
        1,
        &owners,
        29,
    )
    .unwrap_err();

    assert!(error.to_string().contains("second conflicts with first"));
    assert!(error.to_string().contains("LBA 29 user-data offset 0x1"));
}

#[test]
fn sector_patch_plans_compose_without_materialized_disc_candidates() {
    let mut source = crate::disc::form1_sector(&vec![0_u8; USER_DATA_SIZE]);
    regenerate_form1_integrity(&mut source).unwrap();
    let mut first = source;
    first[USER_DATA_OFFSET + 3] = 0x11;
    regenerate_form1_integrity(&mut first).unwrap();
    let mut second = source;
    second[USER_DATA_OFFSET + 9] = 0x22;
    regenerate_form1_integrity(&mut second).unwrap();

    let mut accumulator = SectorPatchAccumulator::default();
    accumulator
        .merge_sector_patches(
            &[SectorPatch {
                lba: 7,
                source_bytes: source,
                bytes: first,
                data_writes: 1,
            }],
            "first",
        )
        .unwrap();
    accumulator
        .merge_sector_patches(
            &[SectorPatch {
                lba: 7,
                source_bytes: source,
                bytes: second,
                data_writes: 1,
            }],
            "second",
        )
        .unwrap();

    let merged = accumulator.into_sector_patches().unwrap();
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].source_bytes, source);
    assert_eq!(merged[0].bytes[USER_DATA_OFFSET + 3], 0x11);
    assert_eq!(merged[0].bytes[USER_DATA_OFFSET + 9], 0x22);
    validate_form1_integrity(&merged[0].bytes).unwrap();
}

#[test]
fn sector_patch_plans_reject_different_immutable_sources() {
    let mut source = crate::disc::form1_sector(&vec![0_u8; USER_DATA_SIZE]);
    regenerate_form1_integrity(&mut source).unwrap();
    let mut first = source;
    first[USER_DATA_OFFSET + 3] = 0x11;
    regenerate_form1_integrity(&mut first).unwrap();

    let mut different_source = source;
    different_source[USER_DATA_OFFSET + 5] = 0x44;
    regenerate_form1_integrity(&mut different_source).unwrap();
    let mut second = different_source;
    second[USER_DATA_OFFSET + 9] = 0x22;
    regenerate_form1_integrity(&mut second).unwrap();

    let mut accumulator = SectorPatchAccumulator::default();
    accumulator
        .merge_sector_patches(
            &[SectorPatch {
                lba: 7,
                source_bytes: source,
                bytes: first,
                data_writes: 1,
            }],
            "first",
        )
        .unwrap();
    let error = accumulator
        .merge_sector_patches(
            &[SectorPatch {
                lba: 7,
                source_bytes: different_source,
                bytes: second,
                data_writes: 1,
            }],
            "second",
        )
        .unwrap_err();

    assert!(error.to_string().contains("source sector changed"));
}

#[test]
fn patched_bin_preserves_every_undeclared_sector() {
    let temporary = tempfile::tempdir().unwrap();
    let source_path = temporary.path().join("source.bin");
    let output_path = temporary.path().join("output.bin");
    let mut sectors = Vec::new();
    for value in [0x11_u8, 0x22, 0x33] {
        let mut raw = crate::disc::form1_sector(&vec![value; USER_DATA_SIZE]);
        regenerate_form1_integrity(&mut raw).unwrap();
        sectors.push(raw);
    }
    let source = sectors.concat();
    fs::write(&source_path, &source).unwrap();

    let mut replacement = sectors[1];
    replacement[USER_DATA_OFFSET + 9] = 0x7f;
    regenerate_form1_integrity(&mut replacement).unwrap();
    let patches = vec![SectorPatch {
        lba: 1,
        source_bytes: sectors[1],
        bytes: replacement,
        data_writes: 1,
    }];

    let output_hash = write_patched_bin(&source_path, &output_path, &patches).unwrap();
    let output = fs::read(&output_path).unwrap();
    assert_eq!(output_hash, hash_file(&output_path).unwrap());
    assert_eq!(&output[..RAW_SECTOR_SIZE], &source[..RAW_SECTOR_SIZE]);
    assert_eq!(
        &output[RAW_SECTOR_SIZE * 2..],
        &source[RAW_SECTOR_SIZE * 2..]
    );
    assert_eq!(
        &output[RAW_SECTOR_SIZE..RAW_SECTOR_SIZE * 2],
        replacement.as_slice()
    );
}
