use super::*;
use crate::executable_storage_gaps::ExecutableStorageGapReviewRegion;

fn candidate(address: u64) -> ScannedStorageGap {
    ScannedStorageGap {
        address,
        file_offset: 0x80 + address - 0x0040_0000,
        bytes: vec![0x82, 0xa0, 0],
        display_text: "あ".to_string(),
        japanese_characters: 1,
    }
}

#[test]
fn review_requires_every_candidate_to_have_one_exact_region() {
    let section = [0x82, 0xa0, 0, 0];
    let catalog = ExecutableStorageGapReviewCatalog {
        schema: 1,
        executable_sha256: "exe".to_string(),
        review_basis: "fixture review".to_string(),
        regions: vec![ExecutableStorageGapReviewRegion {
            region_id: "fixture".to_string(),
            start_address: "0x00400000".to_string(),
            end_address: "0x00400004".to_string(),
            source_sha256: crate::indexed_texture::sha256(&section),
            classification: ExecutableStorageGapClassification::NonStringBinary,
            note: "fixture binary".to_string(),
        }],
    };

    let reviewed = validate_review_catalog(
        &catalog,
        "exe",
        0x0040_0000,
        &section,
        &[candidate(0x0040_0000)],
    )
    .unwrap();

    assert_eq!(reviewed.regions[0].candidate_count, 1);
    assert_eq!(
        reviewed.candidates[0].classification,
        ExecutableStorageGapClassification::NonStringBinary
    );
}

#[test]
fn review_rejects_a_region_without_candidates() {
    let section = [0; 4];
    let catalog = ExecutableStorageGapReviewCatalog {
        schema: 1,
        executable_sha256: "exe".to_string(),
        review_basis: "fixture review".to_string(),
        regions: vec![ExecutableStorageGapReviewRegion {
            region_id: "orphan".to_string(),
            start_address: "0x00400000".to_string(),
            end_address: "0x00400004".to_string(),
            source_sha256: crate::indexed_texture::sha256(&section),
            classification: ExecutableStorageGapClassification::NonStringBinary,
            note: "fixture binary".to_string(),
        }],
    };

    let error = validate_review_catalog(&catalog, "exe", 0x0040_0000, &section, &[]).unwrap_err();

    assert!(error.to_string().contains("region without candidates"));
}

#[test]
fn review_rejects_a_candidate_outside_every_region() {
    let section = [0; 8];
    let catalog = ExecutableStorageGapReviewCatalog {
        schema: 1,
        executable_sha256: "exe".to_string(),
        review_basis: "fixture review".to_string(),
        regions: vec![ExecutableStorageGapReviewRegion {
            region_id: "first-half".to_string(),
            start_address: "0x00400000".to_string(),
            end_address: "0x00400004".to_string(),
            source_sha256: crate::indexed_texture::sha256(&section[..4]),
            classification: ExecutableStorageGapClassification::NonStringBinary,
            note: "fixture binary".to_string(),
        }],
    };

    let error = validate_review_catalog(
        &catalog,
        "exe",
        0x0040_0000,
        &section,
        &[candidate(0x0040_0004)],
    )
    .unwrap_err();

    assert!(error.to_string().contains("has 0 review regions"));
}
