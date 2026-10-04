use super::*;

#[test]
fn source_compatibility_checks_texture_record_roles() {
    let loads = vec![
        LoadRecord {
            flags: 0,
            operation: 1,
            file_record: 0,
            destination: 0,
        },
        LoadRecord {
            flags: 2,
            operation: 2,
            file_record: 1,
            destination: 0,
        },
    ];
    let matching = vec![
        crate::indexed_texture::FileRecord {
            sector_offset: 0,
            sector_count: 1,
            stored_byte_length: 20,
            width: 4,
            height: 5,
        },
        crate::indexed_texture::FileRecord {
            sector_offset: 1,
            sector_count: 1,
            stored_byte_length: 1_024,
            width: 0,
            height: 0,
        },
    ];
    assert!(load_table_matches_source(&loads, &matching));

    let mut wrong_role = matching;
    wrong_role[1].width = 1;
    assert!(!load_table_matches_source(&loads, &wrong_role));
}

#[test]
fn address_materialization_finds_typed_lui_addiu_pair() {
    let source = [0x49, 0x00, 0x05, 0x3c, 0x70, 0x93, 0xa5, 0x24];
    let addresses = BTreeMap::from([(0x0048_9370, "TopMenu_TexTbl".to_string())]);
    let mut references = BTreeMap::new();
    find_address_materializations(
        &source,
        0x0040_0000,
        "TopMenu_Init",
        &addresses,
        &mut references,
    )
    .unwrap();

    assert_eq!(
        references["TopMenu_TexTbl"].iter().next().unwrap(),
        &TextureTableReference {
            owner_symbol: "TopMenu_Init".to_string(),
            address: "0x00400000".to_string(),
            kind: TextureTableReferenceKind::TypedAddressMaterialization,
        }
    );
}

#[test]
fn pointer_scan_records_exact_table_address() {
    let source = 0x0048_9370_u32.to_le_bytes();
    let addresses = BTreeMap::from([(0x0048_9370, "TopMenu_TexTbl".to_string())]);
    let mut references = BTreeMap::new();
    find_pointer_references(
        &source,
        0x0049_0000,
        "TopMenu_Table",
        &addresses,
        &mut references,
    )
    .unwrap();

    assert_eq!(
        references["TopMenu_TexTbl"].iter().next().unwrap().kind,
        TextureTableReferenceKind::Pointer
    );
}

#[test]
fn pointer_table_requires_every_entry_to_name_a_load_table() {
    let addresses = BTreeMap::from([
        (0x0048_ee80, "FrameTexTbl_00".to_string()),
        (0x0048_eed0, "FrameTexTbl_01".to_string()),
    ]);
    let mut source = Vec::new();
    source.extend_from_slice(&0x0048_ee80_u32.to_le_bytes());
    source.extend_from_slice(&0x0048_eed0_u32.to_le_bytes());
    assert_eq!(
        read_pointer_table(&source, &addresses).unwrap().unwrap(),
        ["FrameTexTbl_00", "FrameTexTbl_01"]
    );

    source.extend_from_slice(&0_u32.to_le_bytes());
    assert!(read_pointer_table(&source, &addresses).unwrap().is_none());
}

#[test]
fn empty_load_table_is_only_the_exact_terminator_record() {
    let mut source = Vec::new();
    source.extend_from_slice(&u32::MAX.to_le_bytes());
    source.extend_from_slice(&[0; 12]);
    assert!(is_empty_load_table(&source));

    source[4] = 1;
    assert!(!is_empty_load_table(&source));
}
