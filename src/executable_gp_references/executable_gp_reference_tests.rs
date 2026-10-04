use super::*;

fn decode(word: u32) -> r5900::Instruction {
    r5900::decode_bytes(
        &word.to_le_bytes(),
        r5900::CodeLocation::try_from(0x0040_0000_u64).unwrap(),
    )
    .unwrap()
}

#[test]
fn inline_data_without_a_gp_opcode_is_not_selected_for_typed_decode() {
    let inline_maximum_values = 0x00ff_00ff_u32.to_le_bytes();

    assert!(!is_gp_reference_candidate(&inline_maximum_values));
}

#[test]
fn direct_gp_addiu_resolves_an_exact_candidate() {
    let gp = 0x004f_8370;
    let candidate = gp - 0x100;
    let word = (0x09 << 26) | (28 << 21) | (4 << 16) | 0xff00;
    let candidates = BTreeMap::from([(candidate, "candidate".to_string())]);

    let reference = inspect_gp_reference(&decode(word), gp, 0x0040_0000, &[], &candidates)
        .unwrap()
        .unwrap();

    assert_eq!(reference.candidate_address, candidate);
    assert_eq!(reference.kind, ExecutableGpReferenceKind::DirectAddress);
    assert_eq!(reference.pointer_slot_address, None);
}

#[test]
fn gp_lw_resolves_one_file_backed_pointer_slot() {
    let gp = 0x0040_0100;
    let candidate = 0x0048_ca50_u32;
    let word = (0x23 << 26) | (28 << 21) | (4 << 16) | 0xff80;
    let mut main = vec![0; 0x100];
    main[0x80..0x84].copy_from_slice(&candidate.to_le_bytes());
    let candidates = BTreeMap::from([(candidate, "candidate".to_string())]);

    let reference = inspect_gp_reference(&decode(word), gp, 0x0040_0000, &main, &candidates)
        .unwrap()
        .unwrap();

    assert_eq!(reference.candidate_address, candidate);
    assert_eq!(reference.kind, ExecutableGpReferenceKind::PointerLoad);
    assert_eq!(reference.pointer_slot_address, Some(0x0040_0080));
}

#[test]
fn gp_reference_ignores_an_undeclared_candidate() {
    let gp = 0x004f_8370;
    let word = (0x09 << 26) | (28 << 21) | (4 << 16) | 0xff00;

    let reference =
        inspect_gp_reference(&decode(word), gp, 0x0040_0000, &[], &BTreeMap::new()).unwrap();

    assert_eq!(reference, None);
}
