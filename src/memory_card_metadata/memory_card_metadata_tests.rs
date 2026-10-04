use encoding_rs::SHIFT_JIS;

use super::*;

#[test]
fn exact_shift_jis_title_counts_japanese_and_ascii_runs() {
    let (encoded, _, had_errors) = SHIFT_JIS.encode("ぷよ SAVE つう");
    assert!(!had_errors);
    let mut source = encoded.into_owned();
    source.push(0);

    let (decoded, japanese, ascii_runs) = decode_exact_title(&source).unwrap();

    assert_eq!(decoded, "ぷよ SAVE つう");
    assert_eq!(japanese, 4);
    assert_eq!(ascii_runs, 1);
}

#[test]
fn title_rejects_interior_nul() {
    let error = decode_exact_title(&[0x82, 0xa0, 0, 0]).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("one exact NUL-terminated string")
    );
}
