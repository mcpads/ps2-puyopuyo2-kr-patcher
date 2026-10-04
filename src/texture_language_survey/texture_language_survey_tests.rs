use super::*;
use crate::texture_preview_corpus::{
    ResidualImageReason, ResidualPreviewPair, TexturePreview, TexturePreviewSummary,
};

fn preview(image_file_record: usize, palette_file_record: usize) -> TexturePreview {
    TexturePreview {
        file_id: 0,
        image_container_iso_path: "/IMAGE_D.BIN;1".to_string(),
        image_file_record,
        palette_file_record,
        clear_first_alpha: true,
        owner_tables: vec!["MenuTexTbl".to_string()],
        width: 64,
        height: 32,
        decoded_trailing_byte_length: 0,
        swizzled_indices_sha256: "swizzled".to_string(),
        linear_indices_sha256: format!("linear-{image_file_record}"),
        uploaded_palette_sha256: format!("palette-{palette_file_record}"),
        rgba_sha256: format!("rgba-{palette_file_record}"),
        png_path: format!("preview-{palette_file_record}.png"),
        png_sha256: format!("png-{palette_file_record}"),
    }
}

fn corpus(previews: Vec<TexturePreview>) -> TexturePreviewCorpus {
    let unique_images = previews
        .iter()
        .map(|preview| (preview.file_id, preview.image_file_record))
        .collect::<BTreeSet<_>>()
        .len();
    TexturePreviewCorpus {
        schema: 1,
        source_bin_sha256: "source".to_string(),
        executable_iso_path: "/SLPM_624.00;1".to_string(),
        executable_sha256: "executable".to_string(),
        containers: vec![TexturePreviewContainerIdentity {
            file_id: 0,
            iso_path: "/IMAGE_D.BIN;1".to_string(),
            sha256: "container".to_string(),
        }],
        summary: TexturePreviewSummary {
            load_tables: 1,
            load_order_tables: 1,
            consumer_pair_tables: 0,
            skipped_mismatched_tables: 0,
            unpaired_image_loads: 0,
            pair_instances: previews.len(),
            residual_images: 0,
            residual_preview_variants: 0,
            source_images: unique_images,
            previewed_images: unique_images,
            unique_previews: previews.len(),
        },
        consumer_pair_tables: Vec::new(),
        residual_pairs: Vec::new(),
        skipped_tables: Vec::new(),
        previews,
        evidence_boundary: "fixture".to_string(),
    }
}

fn catalog(groups: Vec<TextureLanguageGroup>) -> TextureLanguageCatalog {
    TextureLanguageCatalog {
        schema: 1,
        source_bin_sha256: "source".to_string(),
        executable_iso_path: "/SLPM_624.00;1".to_string(),
        executable_sha256: "executable".to_string(),
        containers: vec![TexturePreviewContainerIdentity {
            file_id: 0,
            iso_path: "/IMAGE_D.BIN;1".to_string(),
            sha256: "container".to_string(),
        }],
        review_basis: "visual review".to_string(),
        groups,
    }
}

fn group(
    classification: TextureLanguageClassification,
    image_records: Vec<usize>,
) -> TextureLanguageGroup {
    TextureLanguageGroup {
        file_id: 0,
        classification,
        product_scope: ProductScope::Included,
        record_ranges: Vec::new(),
        image_records,
        note: "reviewed".to_string(),
    }
}

#[test]
fn other_title_promotion_is_explicitly_excluded_from_product_targets() {
    let previews = corpus(vec![preview(56, 20)]);
    let mut excluded = group(
        TextureLanguageClassification::JapaneseEnglishMixed,
        vec![56],
    );
    excluded.product_scope = ProductScope::ExcludedOtherTitlePromotion;

    let survey = classify_texture_previews(&previews, &catalog(vec![excluded])).unwrap();

    assert_eq!(survey.summary.japanese_candidate_images, 1);
    assert_eq!(survey.summary.admitted_translation_targets, 0);
    assert_eq!(survey.summary.excluded_other_title_promotion_images, 1);
    assert_eq!(
        survey.images[0].target_decision,
        TextureTargetDecision::ExcludedOtherTitlePromotion
    );
    assert!(!survey.images[0].translation_target);
    assert!(survey.images[0].requires_english_pixel_preservation);
}

#[test]
fn mixed_language_image_collapses_palette_variants_and_protects_english() {
    let previews = corpus(vec![preview(7, 20), preview(7, 21)]);
    let classifications = catalog(vec![group(
        TextureLanguageClassification::JapaneseEnglishMixed,
        vec![7],
    )]);
    let survey = classify_texture_previews(&previews, &classifications).unwrap();
    assert_eq!(survey.summary.preview_variants, 2);
    assert_eq!(survey.summary.unique_images, 1);
    assert_eq!(survey.summary.japanese_candidate_images, 1);
    assert_eq!(survey.summary.admitted_translation_targets, 1);
    assert_eq!(survey.summary.japanese_english_mixed_images, 1);
    assert_eq!(survey.images[0].preview_variants, 2);
    assert!(survey.images[0].translation_target);
    assert!(survey.images[0].requires_english_pixel_preservation);
}

#[test]
fn japanese_residual_is_not_admitted_without_a_consumer_path() {
    let mut previews = corpus(vec![preview(7, 20)]);
    previews.summary.residual_images = 1;
    previews.summary.residual_preview_variants = 1;
    previews.residual_pairs.push(ResidualPreviewPair {
        file_id: 0,
        image_file_record: 7,
        palette_file_record: 20,
        reason: ResidualImageReason::NotInLoadTable,
        palette_load_flags: vec![3],
        preview_alpha_variants: vec![true],
        pairing_basis: "visual survey only".to_string(),
    });
    let classifications = catalog(vec![group(
        TextureLanguageClassification::JapaneseOnly,
        vec![7],
    )]);

    let survey = classify_texture_previews(&previews, &classifications).unwrap();

    assert_eq!(survey.summary.japanese_candidate_images, 1);
    assert_eq!(survey.summary.admitted_translation_targets, 0);
    assert_eq!(survey.summary.excluded_japanese_without_consumer, 1);
    assert_eq!(
        survey.images[0].target_decision,
        TextureTargetDecision::ExcludedNoStaticLoadPath
    );
    assert!(!survey.images[0].translation_target);
}

#[test]
fn catalog_rejects_missing_and_duplicate_image_classifications() {
    let previews = corpus(vec![preview(7, 20), preview(8, 21)]);
    let missing = catalog(vec![group(
        TextureLanguageClassification::JapaneseOnly,
        vec![7],
    )]);
    assert!(classify_texture_previews(&previews, &missing).is_err());

    let duplicate = catalog(vec![
        group(TextureLanguageClassification::JapaneseOnly, vec![7, 8]),
        group(TextureLanguageClassification::NonText, vec![8]),
    ]);
    assert!(classify_texture_previews(&previews, &duplicate).is_err());
}
