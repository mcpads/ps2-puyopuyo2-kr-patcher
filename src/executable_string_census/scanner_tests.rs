use super::*;

#[test]
fn accepts_exact_reversible_shift_jis_with_japanese() {
    let source = [0x83, 0x7b, 0x83, 0x5e, 0x83, 0x93, 0];

    let (text, japanese_characters) = decode_exact_string(&source).unwrap();

    assert_eq!(text, "ボタン");
    assert_eq!(japanese_characters, 3);
}

#[test]
fn rejects_interior_nul_and_noncanonical_storage() {
    assert!(decode_exact_string(b"abc\0def\0").is_none());
    assert!(decode_exact_string(&[0x82, 0]).is_none());
}

#[test]
fn english_only_string_is_exact_but_has_no_japanese_characters() {
    let (text, japanese_characters) = decode_exact_string(b"MEMORY CARD\0").unwrap();

    assert_eq!(text, "MEMORY CARD");
    assert_eq!(japanese_characters, 0);
}
