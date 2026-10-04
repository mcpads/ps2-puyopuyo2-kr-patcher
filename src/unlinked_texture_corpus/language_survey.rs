use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::{UnlinkedTextureCorpus, UnlinkedTexturePreview};
use crate::indexed_texture::sha256;
use crate::texture_language_survey::TextureLanguageClassification;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnlinkedTextureLanguageCatalog {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub container_iso_path: String,
    pub container_sha256: String,
    pub review_basis: String,
    pub images: Vec<UnlinkedTextureLanguageEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnlinkedTextureLanguageEntry {
    pub image_index: usize,
    pub width: u32,
    pub height: u32,
    pub dimension_resolved: bool,
    pub classification: TextureLanguageClassification,
    pub note: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnlinkedTextureLanguageSurvey {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub container_iso_path: String,
    pub container_sha256: String,
    pub review_basis: String,
    pub summary: UnlinkedTextureLanguageSummary,
    pub images: Vec<ClassifiedUnlinkedTextureImage>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct UnlinkedTextureLanguageSummary {
    pub images: usize,
    pub japanese_candidate_images: usize,
    pub admitted_translation_targets: usize,
    pub japanese_only_images: usize,
    pub japanese_english_mixed_images: usize,
    pub english_only_images: usize,
    pub non_text_images: usize,
    pub unresolved_images: usize,
    pub selected_dimension_candidates: usize,
    pub unresolved_dimension_candidates: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClassifiedUnlinkedTextureImage {
    pub image_index: usize,
    pub width: u32,
    pub height: u32,
    pub linear_indices_sha256: String,
    pub png_path: String,
    pub png_sha256: String,
    pub classification: TextureLanguageClassification,
    pub japanese_language_candidate: bool,
    pub translation_target: bool,
    pub requires_english_pixel_preservation: bool,
    pub dimension_resolved: bool,
    pub review_note: String,
}

pub fn verify_unlinked_texture_previews(corpus: &UnlinkedTextureCorpus) -> Result<()> {
    for image in &corpus.images {
        for preview in &image.previews {
            let path = Path::new(&preview.png_path);
            let bytes = std::fs::read(path)
                .with_context(|| format!("failed to read unlinked preview {}", path.display()))?;
            ensure!(
                sha256(&bytes) == preview.png_sha256,
                "unlinked texture preview hash differs: {}",
                path.display()
            );
        }
    }
    Ok(())
}

pub fn classify_unlinked_texture_previews(
    corpus: &UnlinkedTextureCorpus,
    catalog: &UnlinkedTextureLanguageCatalog,
) -> Result<UnlinkedTextureLanguageSurvey> {
    validate_identities(corpus, catalog)?;
    ensure!(
        !catalog.review_basis.trim().is_empty(),
        "unlinked texture review basis is empty"
    );
    let entries = catalog
        .images
        .iter()
        .map(|entry| (entry.image_index, entry))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        entries.len() == catalog.images.len(),
        "unlinked texture catalog has duplicate image indices"
    );
    ensure!(
        entries.keys().copied().eq(0..corpus.images.len()),
        "unlinked texture catalog does not classify every image exactly once"
    );

    let mut summary = UnlinkedTextureLanguageSummary {
        images: corpus.images.len(),
        ..UnlinkedTextureLanguageSummary::default()
    };
    let mut images = Vec::with_capacity(corpus.images.len());
    for image in &corpus.images {
        ensure!(
            image.image_index < corpus.images.len(),
            "unlinked texture image index is out of range"
        );
        let entry = entries[&image.image_index];
        ensure!(
            !entry.note.trim().is_empty(),
            "unlinked texture review note is empty: {}",
            image.image_index
        );
        let preview = selected_preview(image.previews.as_slice(), entry)?;
        if entry.dimension_resolved && image.previews.len() > 1 {
            summary.selected_dimension_candidates += 1;
        }
        if !entry.dimension_resolved {
            ensure!(
                image.previews.len() > 1
                    && entry.classification == TextureLanguageClassification::NonText,
                "only visually indistinguishable non-text dimensions may remain unresolved: {}",
                image.image_index
            );
            summary.unresolved_dimension_candidates += 1;
        }
        if !image.exact_linked_pair_matches.is_empty() {
            ensure!(
                image.exact_linked_pair_matches.iter().any(|matched| {
                    matched.width == entry.width && matched.height == entry.height
                }),
                "selected dimensions differ from the exact linked pair: {}",
                image.image_index
            );
        }
        let (japanese_language_candidate, requires_english_pixel_preservation) =
            language_scope(entry.classification);
        if japanese_language_candidate {
            summary.japanese_candidate_images += 1;
        }
        match entry.classification {
            TextureLanguageClassification::JapaneseOnly => summary.japanese_only_images += 1,
            TextureLanguageClassification::JapaneseEnglishMixed => {
                summary.japanese_english_mixed_images += 1;
            }
            TextureLanguageClassification::EnglishOnly => summary.english_only_images += 1,
            TextureLanguageClassification::NonText => summary.non_text_images += 1,
            TextureLanguageClassification::Unresolved => summary.unresolved_images += 1,
        }
        images.push(ClassifiedUnlinkedTextureImage {
            image_index: image.image_index,
            width: entry.width,
            height: entry.height,
            linear_indices_sha256: preview.linear_indices_sha256.clone(),
            png_path: preview.png_path.clone(),
            png_sha256: preview.png_sha256.clone(),
            classification: entry.classification,
            japanese_language_candidate,
            translation_target: false,
            requires_english_pixel_preservation,
            dimension_resolved: entry.dimension_resolved,
            review_note: entry.note.clone(),
        });
    }
    ensure!(
        summary.japanese_only_images
            + summary.japanese_english_mixed_images
            + summary.english_only_images
            + summary.non_text_images
            + summary.unresolved_images
            == summary.images,
        "unlinked language summary does not partition the image population"
    );
    ensure!(
        summary.admitted_translation_targets == 0,
        "unlinked images cannot become translation targets without a consumer"
    );
    Ok(UnlinkedTextureLanguageSurvey {
        schema: 1,
        source_bin_sha256: corpus.source_bin_sha256.clone(),
        executable_iso_path: corpus.executable_iso_path.clone(),
        executable_sha256: corpus.executable_sha256.clone(),
        container_iso_path: corpus.container_iso_path.clone(),
        container_sha256: corpus.container_sha256.clone(),
        review_basis: catalog.review_basis.clone(),
        summary,
        images,
        evidence_boundary: "Reviewed visible-language and dimension selection for every exact-source image in a CPX container without an ELF file-list owner. Japanese pixels are localization candidates, while English pixels are protected. No image is an admitted translation target until a storage-to-loader-to-registration-to-display consumer path is established. Visually selected dimensions do not prove the runtime texture dimensions.",
    })
}

fn validate_identities(
    corpus: &UnlinkedTextureCorpus,
    catalog: &UnlinkedTextureLanguageCatalog,
) -> Result<()> {
    ensure!(corpus.schema == 1, "unsupported unlinked texture schema");
    ensure!(
        catalog.schema == 1,
        "unsupported unlinked language catalog schema"
    );
    ensure!(
        catalog.source_bin_sha256 == corpus.source_bin_sha256
            && catalog.executable_iso_path == corpus.executable_iso_path
            && catalog.executable_sha256 == corpus.executable_sha256
            && catalog.container_iso_path == corpus.container_iso_path
            && catalog.container_sha256 == corpus.container_sha256,
        "unlinked texture catalog identity differs from its preview corpus"
    );
    Ok(())
}

fn selected_preview<'a>(
    previews: &'a [UnlinkedTexturePreview],
    entry: &UnlinkedTextureLanguageEntry,
) -> Result<&'a UnlinkedTexturePreview> {
    let matches = previews
        .iter()
        .filter(|preview| preview.width == entry.width && preview.height == entry.height)
        .collect::<Vec<_>>();
    let [preview] = matches.as_slice() else {
        anyhow::bail!(
            "catalog dimensions are not one exact preview candidate: {} {}x{}",
            entry.image_index,
            entry.width,
            entry.height
        );
    };
    Ok(*preview)
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

#[cfg(test)]
#[path = "language_survey_tests.rs"]
mod language_survey_tests;
