use crate::memory_card_translation::{KoreanTranslation, TranslationStatus};

use super::ExecutableTextTranslationSource;
use super::validation::{encoded_length_for_test, validate_translation_for_test};

fn source() -> ExecutableTextTranslationSource {
    ExecutableTextTranslationSource {
        asset_id: "puzzle-puyo/test".to_string(),
        consumer_family: "puzzle_puyo".to_string(),
        symbol: "Test".to_string(),
        source_address: "0x00100000".to_string(),
        source_file_offset: "0x00001000".to_string(),
        storage_byte_length: 64,
        payload_byte_length: 63,
        source_sha256: "00".repeat(32),
        source_hex: "00".repeat(64),
        terminator: "nul".to_string(),
        source_text: "DATAをMEMORY CARDに保存してください".to_string(),
        protected_ascii: vec!["DATA".to_string(), "MEMORY CARD".to_string()],
        reference_ids: vec!["test/reference".to_string()],
    }
}

fn tutorial_source() -> ExecutableTextTranslationSource {
    ExecutableTextTranslationSource {
        asset_id: "tutorial/message/test".to_string(),
        consumer_family: "tutorial".to_string(),
        source_text: "一行目\n二行目".to_string(),
        protected_ascii: Vec::new(),
        ..source()
    }
}

fn puzzle_puyo_layout_source() -> ExecutableTextTranslationSource {
    ExecutableTextTranslationSource {
        asset_id: "nazo/window-title/test".to_string(),
        source_text: "転送先の\nデータ位置\nを選んで\n下さい".to_string(),
        protected_ascii: Vec::new(),
        ..source()
    }
}

fn translation(status: TranslationStatus, text: &str) -> KoreanTranslation {
    KoreanTranslation {
        status,
        text: text.to_string(),
        translator_notes: Vec::new(),
        human_approval: None,
        independent_review: None,
    }
}

#[test]
fn accepts_review_draft_with_preserved_ascii() {
    validate_translation_for_test(
        &source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "DATA를 MEMORY CARD에 저장합니다",
        ),
    )
    .unwrap();
}

#[test]
fn rejects_removed_or_reordered_ascii() {
    let error = validate_translation_for_test(
        &source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "MEMORY CARD의 DATA를 저장합니다",
        ),
    )
    .unwrap_err();
    assert!(error.to_string().contains("missing or reordered"));
}

#[test]
fn rejects_japanese_in_review_ready_text() {
    let error = validate_translation_for_test(
        &source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "DATA를 MEMORY CARDに 저장합니다",
        ),
    )
    .unwrap_err();
    assert!(error.to_string().contains("outside ASCII and Hangul"));
}

#[test]
fn rejects_complete_before_runtime_gate_exists() {
    let error = validate_translation_for_test(
        &source(),
        &translation(
            TranslationStatus::Complete,
            "DATA를 MEMORY CARD에 저장합니다",
        ),
    )
    .unwrap_err();
    assert!(error.to_string().contains("complete status is unavailable"));
}

#[test]
fn fixed_storage_length_counts_ascii_hangul_newline_and_nul() {
    assert_eq!(
        encoded_length_for_test("puzzle_puyo", "DATA\n퍼즐뿌요").unwrap(),
        14
    );
}

#[test]
fn fixed_storage_length_rejects_unmapped_characters() {
    let error = encoded_length_for_test("puzzle_puyo", "퍼즐뿌요☆").unwrap_err();
    assert!(error.to_string().contains("cannot be encoded"));
}

#[test]
fn tutorial_storage_counts_space_as_a_two_byte_fullwidth_glyph() {
    assert_eq!(
        encoded_length_for_test("tutorial", "뿌요 이동").unwrap(),
        11
    );
}

#[test]
fn tutorial_storage_rejects_ascii_without_a_two_byte_mapping() {
    let error = encoded_length_for_test("tutorial", "사라져.").unwrap_err();
    assert!(error.to_string().contains("two-byte renderer encoding"));
}

#[test]
fn tutorial_layout_accepts_nine_cells_per_line() {
    validate_translation_for_test(
        &tutorial_source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "가나다라마바사아자\n사라져",
        ),
    )
    .unwrap();
}

#[test]
fn tutorial_layout_rejects_a_line_wider_than_the_window() {
    let error = validate_translation_for_test(
        &tutorial_source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "가나다라마바사아자차\n사라져",
        ),
    )
    .unwrap_err();
    assert!(error.to_string().contains("window holds 9"));
}

#[test]
fn tutorial_layout_accepts_eight_lines_independent_of_japanese_line_count() {
    validate_translation_for_test(
        &tutorial_source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "하나\n둘\n셋\n넷\n다섯\n여섯\n일곱\n여덟",
        ),
    )
    .unwrap();
}

#[test]
fn tutorial_layout_rejects_a_ninth_line() {
    let error = validate_translation_for_test(
        &tutorial_source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "하나\n둘\n셋\n넷\n다섯\n여섯\n일곱\n여덟\n아홉",
        ),
    )
    .unwrap_err();
    assert!(error.to_string().contains("consumer holds 8"));
}

#[test]
fn puzzle_puyo_layout_accepts_reflow_within_source_width_and_line_count() {
    validate_translation_for_test(
        &puzzle_puyo_layout_source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "복사할\n데이터\n위치를\n골라주세요",
        ),
    )
    .unwrap();
}

#[test]
fn puzzle_puyo_layout_rejects_a_line_wider_than_the_source_window() {
    let error = validate_translation_for_test(
        &puzzle_puyo_layout_source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "복사할\n데이터 위치를\n골라\n주세요",
        ),
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("uses 13 halfwidth cells but the source-backed window allocation holds 10")
    );
}

#[test]
fn puzzle_puyo_layout_rejects_more_lines_than_the_source_allocation() {
    let error = validate_translation_for_test(
        &puzzle_puyo_layout_source(),
        &translation(
            TranslationStatus::NeedsHumanReview,
            "복사할\n데이터\n위치를\n골라\n주세요",
        ),
    )
    .unwrap_err();

    assert!(error.to_string().contains("source window allocation has 4"));
}
