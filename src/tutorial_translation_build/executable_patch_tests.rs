use super::*;

#[test]
fn tutorial_codec_uses_fullwidth_space_and_preserves_newline_and_final_nul() {
    let codes = BTreeMap::from([('한', [0xf0, 0x40]), ('글', [0xf0, 0x41])]);

    assert_eq!(
        encode_translation("한 글\n한", &codes).unwrap(),
        [0xf0, 0x40, 0x81, 0x40, 0xf0, 0x41, b'\n', 0xf0, 0x40, 0]
    );
}

#[test]
fn tutorial_codec_rejects_one_byte_ascii_that_would_desynchronize_the_renderer() {
    let error = encode_translation("한.", &BTreeMap::from([('한', [0xf0, 0x40])])).unwrap_err();

    assert!(error.to_string().contains("two-byte renderer encoding"));
}

#[test]
fn tutorial_codec_rejects_an_unassigned_hangul_syllable() {
    let error = encode_translation("한글", &BTreeMap::from([('한', [0xf0, 0x40])])).unwrap_err();

    assert!(error.to_string().contains("unassigned glyph: 글"));
}
