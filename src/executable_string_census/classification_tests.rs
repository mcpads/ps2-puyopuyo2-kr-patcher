use super::*;
use crate::executable_string_census::{
    ExecutableStringReviewEntry, ReviewedCandidateClassification,
};

fn candidate(symbol: &str, address: u32, bytes: &[u8]) -> ScannedJapaneseString {
    ScannedJapaneseString {
        symbol: symbol.to_string(),
        address,
        file_offset: 0,
        bytes: bytes.to_vec(),
        display_text: "あ".to_string(),
        japanese_characters: 1,
    }
}

fn catalog(entry: ExecutableStringReviewEntry) -> ExecutableStringReviewCatalog {
    ExecutableStringReviewCatalog {
        schema: 1,
        executable_sha256: "executable".to_string(),
        review_basis: "source-bound structural review".to_string(),
        entries: vec![entry],
    }
}

#[test]
fn reviewed_metadata_becomes_a_japanese_translation_target() {
    let candidate = candidate("metadata", 0x0040_1000, &[0x82, 0xa0, 0]);
    let review = ExecutableStringReviewEntry {
        symbol: candidate.symbol.clone(),
        address: "0x00401000".to_string(),
        source_sha256: crate::indexed_texture::sha256(&candidate.bytes),
        classification: ReviewedCandidateClassification::DisplayedJapaneseMetadata,
        note: "system-displayed title".to_string(),
    };

    let decisions = classify_candidates(&[candidate], &catalog(review), "executable").unwrap();

    assert!(decisions["metadata"].translation_target);
    assert_eq!(
        decisions["metadata"].classification,
        ExecutableStringClassification::DisplayedJapaneseMetadata
    );
}

#[test]
fn uncovered_candidate_fails_closed() {
    let catalog = ExecutableStringReviewCatalog {
        schema: 1,
        executable_sha256: "executable".to_string(),
        review_basis: "source-bound structural review".to_string(),
        entries: Vec::new(),
    };

    let error = classify_candidates(
        &[candidate("unreviewed", 0x0040_1000, &[0x82, 0xa0, 0])],
        &catalog,
        "executable",
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("has no message membership or review")
    );
}

#[test]
fn review_hash_is_part_of_the_candidate_identity() {
    let candidate = candidate("binary", 0x0040_1000, &[0x82, 0xa0, 0]);
    let review = ExecutableStringReviewEntry {
        symbol: candidate.symbol.clone(),
        address: "0x00401000".to_string(),
        source_sha256: "wrong".to_string(),
        classification: ReviewedCandidateClassification::NonStringBinary,
        note: "pointer bytes".to_string(),
    };

    let error = classify_candidates(&[candidate], &catalog(review), "executable").unwrap_err();

    assert!(error.to_string().contains("review source hash differs"));
}
