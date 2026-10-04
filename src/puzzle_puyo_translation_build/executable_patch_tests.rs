use super::*;

#[test]
fn condition_layout_reserves_dynamic_number_and_icon_cells() {
    assert_eq!(
        preserve_question_indentation(
            "　　匹（　　以外）\n　同時に消すべし",
            "    개(    제외)\n동시에 지울 것"
        )
        .unwrap(),
        "    개(     제외)\n  동시에 지울 것"
    );
    assert_eq!(
        preserve_question_indentation("　　連鎖するべし", "연쇄할 것").unwrap(),
        "    연쇄할 것"
    );
    assert_eq!(
        preserve_question_indentation("　　　　ぷよ\n　　全部消すべし", "뿌요\n전부 지울 것")
            .unwrap(),
        "        뿌요\n    전부 지울 것"
    );
    assert_eq!(
        preserve_question_indentation("　　ぷよを\n　　　匹消すべし", "뿌요를\n    개 지울 것")
            .unwrap(),
        "    뿌요를\n      개 지울 것"
    );
    assert!(preserve_question_indentation("　　ぷよ\n　全部", "뿌요 전부").is_err());
}

#[test]
fn puzzle_puyo_codec_preserves_ascii_newline_and_final_nul() {
    let codes = BTreeMap::from([('한', [0xf0, 0x40]), ('글', [0xf0, 0x41])]);

    assert_eq!(
        encode_translation("한글\nA", &codes).unwrap(),
        [0xf0, 0x40, 0xf0, 0x41, b'\n', b'A', 0]
    );
}

#[test]
fn puzzle_puyo_codec_rejects_an_unassigned_hangul_syllable() {
    let error = encode_translation("한글", &BTreeMap::from([('한', [0xf0, 0x40])])).unwrap_err();

    assert!(error.to_string().contains("unassigned glyph: 글"));
}
