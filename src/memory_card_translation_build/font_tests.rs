use super::*;

#[test]
fn glyph_demand_follows_first_appearance_across_the_selected_corpus() {
    let glyphs = collect_unique_hangul(["가나다가", "라마나"]).unwrap();

    assert_eq!(glyphs, ['가', '나', '다', '라', '마']);
}

#[test]
fn encoding_preserves_ascii_controls_and_uses_assigned_two_byte_codes() {
    let assignments = [MemoryCardGlyphAssignment {
        character: "한".to_string(),
        code_hex: "f040".to_string(),
        slot_index: 0,
        x: 0,
        y: 0,
    }];

    assert_eq!(
        encode_translation("MEMORY CARD@P\n한", &assignments).unwrap(),
        b"MEMORY CARD@P\n\xf0\x40\0"
    );
}

#[test]
fn encoding_rejects_a_character_without_an_assigned_glyph() {
    let error = encode_translation("한글", &[]).unwrap_err().to_string();

    assert!(error.contains("unassigned glyph"));
}
