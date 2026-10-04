use super::capacity::{
    encoded_translation_byte_length_for_test, measure_translation_capacity_for_test,
};
use super::validation::validate_translation_for_test;
use super::{KoreanTranslation, MemoryCardTranslationSource, TranslationStatus};

fn source() -> MemoryCardTranslationSource {
    MemoryCardTranslationSource {
        message_id: "memory-card/test".to_string(),
        symbol: "Message".to_string(),
        source_address: "0x00000000".to_string(),
        source_file_offset: "0x00000000".to_string(),
        storage_byte_length: 1,
        payload_byte_length: 0,
        source_sha256: "0".repeat(64),
        source_hex: "00".to_string(),
        terminator: "00".to_string(),
        source_text: "MEMORY CARD@P".to_string(),
        protected_ascii: vec!["MEMORY CARD".to_string()],
        protected_runtime_controls: vec!["@P".to_string()],
        consumer_corpora: vec!["memory_card_load".to_string()],
        reference_ids: vec!["reference".to_string()],
    }
}

fn review_translation(text: &str) -> KoreanTranslation {
    KoreanTranslation {
        status: TranslationStatus::NeedsReview,
        text: text.to_string(),
        translator_notes: Vec::new(),
        human_approval: None,
        independent_review: None,
    }
}

#[test]
fn accepts_layout_changes_while_preserving_english_and_runtime_control() {
    validate_translation_for_test(
        &source(),
        &review_translation("MEMORY CARD 슬롯@P\n확인 중"),
    )
    .unwrap();
}

#[test]
fn rejects_changed_runtime_control() {
    let error = validate_translation_for_test(
        &source(),
        &review_translation("MEMORY CARD 슬롯@A\n확인 중"),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("runtime controls changed"));
}

#[test]
fn rejects_removed_protected_english() {
    let error = validate_translation_for_test(&source(), &review_translation("메모리 카드 슬롯@P"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("protected ASCII run"));
}

#[test]
fn rejects_review_ready_japanese_text() {
    let error = validate_translation_for_test(
        &source(),
        &review_translation("MEMORY CARD メモリーカード@P"),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("Japanese kana"));
}

#[test]
fn untranslated_state_cannot_hide_translation_text() {
    let mut translation = review_translation("MEMORY CARD 슬롯@P");
    translation.status = TranslationStatus::Untranslated;
    assert!(validate_translation_for_test(&source(), &translation).is_err());
}

#[test]
fn review_pending_translation_can_pass_mechanical_validation() {
    let mut translation = review_translation("MEMORY CARD 슬롯@P");
    translation.status = TranslationStatus::NeedsHumanReview;
    validate_translation_for_test(&source(), &translation).unwrap();
}

#[test]
fn complete_status_cannot_bypass_unrepresented_release_candidate_gates() {
    let mut translation = review_translation("MEMORY CARD 슬롯@P");
    translation.status = TranslationStatus::Complete;
    assert!(validate_translation_for_test(&source(), &translation).is_err());
}

#[test]
fn glyph_capacity_counts_shared_hangul_once_across_messages() {
    let capacity = measure_translation_capacity_for_test(&["가나다", "다라마"], 5).unwrap();
    assert_eq!(capacity.required_unique_hangul_glyphs, 5);
    assert_eq!(capacity.remaining_reassignable_slots, 0);
}

#[test]
fn glyph_capacity_rejects_demand_beyond_proven_slots() {
    assert!(measure_translation_capacity_for_test(&["가나다"], 2).is_err());
}

#[test]
fn glyph_capacity_rejects_unmapped_non_ascii_characters() {
    let error = measure_translation_capacity_for_test(&["확인…"], 33)
        .unwrap_err()
        .to_string();
    assert!(error.contains("outside the supported Hangul syllable range"));
}

#[test]
fn fixed_storage_length_counts_ascii_hangul_and_final_nul() {
    assert_eq!(
        encoded_translation_byte_length_for_test("A가\n").unwrap(),
        5
    );
}

#[test]
fn fixed_storage_length_rejects_characters_without_an_encoding() {
    assert!(encoded_translation_byte_length_for_test("확인…").is_err());
}
