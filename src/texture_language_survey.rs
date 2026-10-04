use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::indexed_texture::sha256;
use crate::product_scope::ProductScope;
use crate::texture_preview_corpus::{
    ResidualImageReason, ResidualPreviewPair, TexturePreview, TexturePreviewContainerIdentity,
    TexturePreviewCorpus,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextureLanguageCatalog {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub containers: Vec<TexturePreviewContainerIdentity>,
    pub review_basis: String,
    pub groups: Vec<TextureLanguageGroup>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextureLanguageGroup {
    pub file_id: usize,
    pub classification: TextureLanguageClassification,
    #[serde(default)]
    pub product_scope: ProductScope,
    #[serde(default)]
    pub record_ranges: Vec<InclusiveRecordRange>,
    #[serde(default)]
    pub image_records: Vec<usize>,
    pub note: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InclusiveRecordRange {
    pub first: usize,
    pub last: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureLanguageClassification {
    JapaneseOnly,
    JapaneseEnglishMixed,
    EnglishOnly,
    NonText,
    Unresolved,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureLanguageSurvey {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub containers: Vec<TexturePreviewContainerIdentity>,
    pub review_basis: String,
    pub summary: TextureLanguageSummary,
    pub images: Vec<ClassifiedTextureImage>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TextureLanguageSummary {
    pub preview_variants: usize,
    pub unique_images: usize,
    pub consumer_pair_tables: usize,
    pub unpaired_image_loads: usize,
    pub residual_images: usize,
    pub source_images: usize,
    pub japanese_candidate_images: usize,
    pub admitted_translation_targets: usize,
    pub excluded_japanese_without_consumer: usize,
    pub excluded_other_title_promotion_images: usize,
    pub japanese_only_images: usize,
    pub japanese_english_mixed_images: usize,
    pub english_only_images: usize,
    pub non_text_images: usize,
    pub unresolved_images: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClassifiedTextureImage {
    pub file_id: usize,
    pub image_container_iso_path: String,
    pub image_file_record: usize,
    pub width: u32,
    pub height: u32,
    pub linear_indices_sha256: String,
    pub preview_variants: usize,
    pub owner_tables: Vec<String>,
    pub residual_pair: Option<ResidualPreviewPair>,
    pub classification: TextureLanguageClassification,
    pub product_scope: ProductScope,
    pub japanese_language_candidate: bool,
    pub target_decision: TextureTargetDecision,
    pub translation_target: bool,
    pub requires_english_pixel_preservation: bool,
    pub review_note: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureTargetDecision {
    AdmittedStaticLoadPath,
    ExcludedNoStaticLoadPath,
    ExcludedNoRegisteredConsumer,
    ExcludedOtherTitlePromotion,
    OutsideJapaneseScope,
    UnresolvedLanguage,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ImageKey {
    file_id: usize,
    image_file_record: usize,
}

struct PreviewImage<'a> {
    first: &'a TexturePreview,
    variants: usize,
    owner_tables: BTreeSet<String>,
}

pub fn verify_texture_preview_files(corpus: &TexturePreviewCorpus) -> Result<()> {
    for preview in &corpus.previews {
        let path = Path::new(&preview.png_path);
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read texture preview {}", path.display()))?;
        ensure!(
            sha256(&bytes) == preview.png_sha256,
            "texture preview hash differs: {}",
            path.display()
        );
    }
    Ok(())
}

pub fn classify_texture_previews(
    corpus: &TexturePreviewCorpus,
    catalog: &TextureLanguageCatalog,
) -> Result<TextureLanguageSurvey> {
    validate_identities(corpus, catalog)?;
    ensure!(
        !catalog.review_basis.trim().is_empty(),
        "texture language review basis is empty"
    );
    let preview_images = collect_preview_images(corpus)?;
    validate_corpus_summary(corpus, preview_images.len())?;
    let classifications = expand_catalog(catalog, preview_images.keys().copied().collect())?;
    let residual_pairs = corpus
        .residual_pairs
        .iter()
        .map(|pair| ((pair.file_id, pair.image_file_record), pair))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        residual_pairs.len() == corpus.residual_pairs.len(),
        "texture preview corpus has duplicate residual image evidence"
    );
    let mut summary = TextureLanguageSummary {
        preview_variants: corpus.previews.len(),
        unique_images: preview_images.len(),
        consumer_pair_tables: corpus.summary.consumer_pair_tables,
        unpaired_image_loads: corpus.summary.unpaired_image_loads,
        residual_images: corpus.summary.residual_images,
        source_images: corpus.summary.source_images,
        ..TextureLanguageSummary::default()
    };
    let mut images = Vec::with_capacity(preview_images.len());
    for (key, preview_image) in preview_images {
        let (classification, product_scope, review_note) = classifications
            .get(&key)
            .context("classified texture image is missing from the catalog")?;
        let (japanese_language_candidate, requires_english_pixel_preservation) =
            language_scope(*classification);
        let residual_pair = residual_pairs.get(&(key.file_id, key.image_file_record));
        let target_decision =
            target_decision(*classification, *product_scope, residual_pair.copied())?;
        let translation_target = target_decision == TextureTargetDecision::AdmittedStaticLoadPath;
        if japanese_language_candidate {
            summary.japanese_candidate_images += 1;
        }
        if translation_target {
            summary.admitted_translation_targets += 1;
        }
        if matches!(
            target_decision,
            TextureTargetDecision::ExcludedNoStaticLoadPath
                | TextureTargetDecision::ExcludedNoRegisteredConsumer
        ) {
            summary.excluded_japanese_without_consumer += 1;
        }
        if target_decision == TextureTargetDecision::ExcludedOtherTitlePromotion {
            summary.excluded_other_title_promotion_images += 1;
        }
        match classification {
            TextureLanguageClassification::JapaneseOnly => summary.japanese_only_images += 1,
            TextureLanguageClassification::JapaneseEnglishMixed => {
                summary.japanese_english_mixed_images += 1;
            }
            TextureLanguageClassification::EnglishOnly => summary.english_only_images += 1,
            TextureLanguageClassification::NonText => summary.non_text_images += 1,
            TextureLanguageClassification::Unresolved => summary.unresolved_images += 1,
        }
        images.push(ClassifiedTextureImage {
            file_id: key.file_id,
            image_container_iso_path: preview_image.first.image_container_iso_path.clone(),
            image_file_record: key.image_file_record,
            width: preview_image.first.width,
            height: preview_image.first.height,
            linear_indices_sha256: preview_image.first.linear_indices_sha256.clone(),
            preview_variants: preview_image.variants,
            owner_tables: preview_image.owner_tables.into_iter().collect(),
            residual_pair: residual_pair.copied().cloned(),
            classification: *classification,
            product_scope: *product_scope,
            japanese_language_candidate,
            target_decision,
            translation_target,
            requires_english_pixel_preservation,
            review_note: review_note.clone(),
        });
    }
    ensure!(
        summary.japanese_only_images
            + summary.japanese_english_mixed_images
            + summary.english_only_images
            + summary.non_text_images
            + summary.unresolved_images
            == summary.unique_images,
        "texture language summary does not partition the preview population"
    );
    ensure!(
        summary.admitted_translation_targets
            + summary.excluded_japanese_without_consumer
            + summary.excluded_other_title_promotion_images
            == summary.japanese_candidate_images,
        "Japanese texture candidates are not fully admitted or explicitly excluded"
    );
    Ok(TextureLanguageSurvey {
        schema: 1,
        source_bin_sha256: corpus.source_bin_sha256.clone(),
        executable_iso_path: corpus.executable_iso_path.clone(),
        executable_sha256: corpus.executable_sha256.clone(),
        containers: corpus.containers.clone(),
        review_basis: catalog.review_basis.clone(),
        summary,
        images,
        evidence_boundary: "This is a reviewed classification of every exact-source D/G/MD image record. Japanese-only and mixed images are language candidates, but only candidates with a validated static load-table path and included product scope are admitted translation targets. A residual preview does not prove loading or registration, and other-title promotional catalog images remain explicitly excluded. English pixels remain protected. The survey does not cover IMAGE_E, video, runtime reachability, or translation approval.",
    })
}

fn validate_corpus_summary(corpus: &TexturePreviewCorpus, unique_images: usize) -> Result<()> {
    ensure!(
        corpus.summary.unique_previews == corpus.previews.len(),
        "texture preview count differs from its summary"
    );
    ensure!(
        corpus.summary.previewed_images == unique_images,
        "texture preview image count differs from its summary"
    );
    ensure!(
        corpus.summary.previewed_images == corpus.summary.source_images,
        "texture preview corpus does not cover every declared source image"
    );
    ensure!(
        corpus.summary.residual_images == corpus.residual_pairs.len(),
        "residual image count differs from its summary"
    );
    ensure!(
        corpus.summary.residual_preview_variants
            == corpus
                .residual_pairs
                .iter()
                .map(|pair| pair.preview_alpha_variants.len())
                .sum::<usize>(),
        "residual preview variant count differs from its summary"
    );
    ensure!(
        corpus.summary.unpaired_image_loads
            == corpus
                .consumer_pair_tables
                .iter()
                .map(|table| table.unpaired_image_load_indices.len())
                .sum::<usize>(),
        "unpaired image load count differs from its summary"
    );
    Ok(())
}

fn validate_identities(
    corpus: &TexturePreviewCorpus,
    catalog: &TextureLanguageCatalog,
) -> Result<()> {
    ensure!(corpus.schema == 1, "unsupported texture preview schema");
    ensure!(
        catalog.schema == 1,
        "unsupported texture language catalog schema"
    );
    ensure!(
        catalog.source_bin_sha256 == corpus.source_bin_sha256,
        "texture language catalog source BIN differs from preview corpus"
    );
    ensure!(
        catalog.executable_iso_path == corpus.executable_iso_path,
        "texture language catalog executable path differs from preview corpus"
    );
    ensure!(
        catalog.executable_sha256 == corpus.executable_sha256,
        "texture language catalog executable hash differs from preview corpus"
    );
    ensure!(
        catalog.containers == corpus.containers,
        "texture language catalog container identities differ from preview corpus"
    );
    Ok(())
}

fn collect_preview_images(
    corpus: &TexturePreviewCorpus,
) -> Result<BTreeMap<ImageKey, PreviewImage<'_>>> {
    let mut images = BTreeMap::<ImageKey, PreviewImage<'_>>::new();
    for preview in &corpus.previews {
        let key = ImageKey {
            file_id: preview.file_id,
            image_file_record: preview.image_file_record,
        };
        if let Some(image) = images.get_mut(&key) {
            ensure!(
                image.first.image_container_iso_path == preview.image_container_iso_path
                    && image.first.width == preview.width
                    && image.first.height == preview.height
                    && image.first.linear_indices_sha256 == preview.linear_indices_sha256,
                "preview variants disagree about image identity for file {} record {}",
                key.file_id,
                key.image_file_record
            );
            image.variants += 1;
            image
                .owner_tables
                .extend(preview.owner_tables.iter().cloned());
        } else {
            images.insert(
                key,
                PreviewImage {
                    first: preview,
                    variants: 1,
                    owner_tables: preview.owner_tables.iter().cloned().collect(),
                },
            );
        }
    }
    ensure!(!images.is_empty(), "texture preview corpus is empty");
    Ok(images)
}

fn expand_catalog(
    catalog: &TextureLanguageCatalog,
    expected: BTreeSet<ImageKey>,
) -> Result<BTreeMap<ImageKey, (TextureLanguageClassification, ProductScope, String)>> {
    let mut classifications = BTreeMap::new();
    for group in &catalog.groups {
        ensure!(
            !group.note.trim().is_empty(),
            "texture language group note is empty for file {}",
            group.file_id
        );
        let mut records = group.image_records.clone();
        for range in &group.record_ranges {
            ensure!(
                range.first <= range.last,
                "texture record range is reversed"
            );
            records.extend(range.first..=range.last);
        }
        ensure!(
            !records.is_empty(),
            "texture language group has no image records"
        );
        for image_file_record in records {
            let key = ImageKey {
                file_id: group.file_id,
                image_file_record,
            };
            ensure!(
                expected.contains(&key),
                "texture language catalog names an unknown image: file {} record {}",
                key.file_id,
                key.image_file_record
            );
            ensure!(
                classifications
                    .insert(
                        key,
                        (
                            group.classification,
                            group.product_scope,
                            group.note.clone()
                        ),
                    )
                    .is_none(),
                "texture language catalog classifies an image more than once: file {} record {}",
                key.file_id,
                key.image_file_record
            );
        }
    }
    let classified = classifications.keys().copied().collect::<BTreeSet<_>>();
    let missing = expected
        .difference(&classified)
        .copied()
        .collect::<Vec<_>>();
    ensure!(
        missing.is_empty(),
        "texture language catalog is missing {} preview images; first missing is file {} record {}",
        missing.len(),
        missing.first().map_or(0, |key| key.file_id),
        missing.first().map_or(0, |key| key.image_file_record)
    );
    Ok(classifications)
}

fn language_scope(classification: TextureLanguageClassification) -> (bool, bool) {
    match classification {
        TextureLanguageClassification::JapaneseOnly => (true, false),
        TextureLanguageClassification::JapaneseEnglishMixed => (true, true),
        TextureLanguageClassification::EnglishOnly => (false, true),
        TextureLanguageClassification::NonText | TextureLanguageClassification::Unresolved => {
            (false, false)
        }
    }
}

fn target_decision(
    classification: TextureLanguageClassification,
    product_scope: ProductScope,
    residual_pair: Option<&ResidualPreviewPair>,
) -> Result<TextureTargetDecision> {
    if !product_scope.is_included() {
        ensure!(
            matches!(
                classification,
                TextureLanguageClassification::JapaneseOnly
                    | TextureLanguageClassification::JapaneseEnglishMixed
            ) && residual_pair.is_none(),
            "product-scope texture exclusion must name a consumer-linked Japanese candidate"
        );
        return Ok(TextureTargetDecision::ExcludedOtherTitlePromotion);
    }
    Ok(match classification {
        TextureLanguageClassification::Unresolved => TextureTargetDecision::UnresolvedLanguage,
        TextureLanguageClassification::EnglishOnly | TextureLanguageClassification::NonText => {
            TextureTargetDecision::OutsideJapaneseScope
        }
        TextureLanguageClassification::JapaneseOnly
        | TextureLanguageClassification::JapaneseEnglishMixed => match residual_pair {
            None => TextureTargetDecision::AdmittedStaticLoadPath,
            Some(pair) => match &pair.reason {
                ResidualImageReason::NotInLoadTable => {
                    TextureTargetDecision::ExcludedNoStaticLoadPath
                }
                ResidualImageReason::LoadedButNotRegistered { .. } => {
                    TextureTargetDecision::ExcludedNoRegisteredConsumer
                }
            },
        },
    })
}

#[cfg(test)]
#[path = "texture_language_survey/texture_language_survey_tests.rs"]
mod texture_language_survey_tests;
