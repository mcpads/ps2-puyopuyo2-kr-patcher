use anyhow::{Result, ensure};

use super::{
    ExcludedJapaneseCandidate, ExecutableTextTarget, LinkedTextureTarget, MovieTarget,
    TranslationExclusionReason, TranslationSurface, TranslationTargetInputs,
};
use crate::executable_string_census::ExecutableStringClassification;
use crate::movie_corpus::MovieLanguageClassification;
use crate::product_scope::ProductScope;
use crate::texture_language_survey::TextureTargetDecision;

pub(super) fn collect_executable_targets(
    inputs: &TranslationTargetInputs<'_>,
) -> Vec<ExecutableTextTarget> {
    inputs
        .executable_strings
        .candidates
        .iter()
        .filter(|candidate| candidate.translation_target)
        .map(|candidate| ExecutableTextTarget {
            symbol: candidate.symbol.clone(),
            address: candidate.address.clone(),
            storage_byte_length: candidate.storage_byte_length,
            source_sha256: candidate.source_sha256.clone(),
            japanese_characters: candidate.japanese_characters,
            classification: candidate.classification,
            declared_message_corpora: candidate.declared_message_corpora.clone(),
            translation_policy: candidate.translation_policy.clone(),
        })
        .collect()
}

pub(super) fn collect_linked_texture_targets(
    inputs: &TranslationTargetInputs<'_>,
) -> Vec<LinkedTextureTarget> {
    inputs
        .linked_textures
        .images
        .iter()
        .filter(|image| image.translation_target)
        .map(|image| LinkedTextureTarget {
            container_iso_path: image.image_container_iso_path.clone(),
            image_file_record: image.image_file_record,
            width: image.width,
            height: image.height,
            linear_indices_sha256: image.linear_indices_sha256.clone(),
            classification: image.classification,
            requires_english_pixel_preservation: image.requires_english_pixel_preservation,
            owner_tables: image.owner_tables.clone(),
        })
        .collect()
}

pub(super) fn collect_movie_targets(inputs: &TranslationTargetInputs<'_>) -> Vec<MovieTarget> {
    inputs
        .movies
        .movies
        .iter()
        .filter(|movie| movie.translation_target)
        .map(|movie| MovieTarget {
            iso_path: movie.iso_path.clone(),
            sha256: movie.sha256.clone(),
            frames: movie.stream.frames,
            classification: movie.classification,
            requires_english_pixel_preservation: movie.requires_english_pixel_preservation,
        })
        .collect()
}

pub(super) fn collect_exclusions(
    inputs: &TranslationTargetInputs<'_>,
) -> Result<Vec<ExcludedJapaneseCandidate>> {
    let mut exclusions = collect_executable_exclusions(inputs)?;
    exclusions.extend(collect_linked_texture_exclusions(inputs)?);
    exclusions.extend(collect_unlinked_texture_exclusions(inputs)?);
    exclusions.extend(collect_movie_exclusions(inputs)?);
    Ok(exclusions)
}

fn collect_movie_exclusions(
    inputs: &TranslationTargetInputs<'_>,
) -> Result<Vec<ExcludedJapaneseCandidate>> {
    inputs
        .movies
        .movies
        .iter()
        .filter(|movie| {
            matches!(
                movie.classification,
                MovieLanguageClassification::JapaneseOnly
                    | MovieLanguageClassification::JapaneseEnglishMixed
            ) && !movie.translation_target
        })
        .map(|movie| {
            ensure!(
                movie.product_scope == ProductScope::ExcludedOtherTitlePromotion,
                "Japanese PSS was excluded without an explicit product-scope reason: {}",
                movie.iso_path
            );
            Ok(ExcludedJapaneseCandidate {
                surface: TranslationSurface::Movie,
                locator: movie.iso_path.clone(),
                reason: TranslationExclusionReason::OtherTitlePromotion,
                review_note: movie.review_note.clone(),
            })
        })
        .collect()
}

fn collect_executable_exclusions(
    inputs: &TranslationTargetInputs<'_>,
) -> Result<Vec<ExcludedJapaneseCandidate>> {
    inputs
        .executable_strings
        .candidates
        .iter()
        .filter(|candidate| !candidate.translation_target)
        .map(|candidate| {
            let reason = match candidate.classification {
                ExecutableStringClassification::FontRepertoire => {
                    TranslationExclusionReason::FontRepertoireNotDisplayedText
                }
                ExecutableStringClassification::NonStringBinary => {
                    TranslationExclusionReason::BinaryFalsePositive
                }
                ExecutableStringClassification::DeclaredMessage
                | ExecutableStringClassification::DisplayedJapaneseMetadata => {
                    anyhow::bail!(
                        "displayed executable Japanese was excluded: {}",
                        candidate.symbol
                    )
                }
            };
            Ok(ExcludedJapaneseCandidate {
                surface: TranslationSurface::ExecutableStorage,
                locator: format!("{}@{}", candidate.symbol, candidate.address),
                reason,
                review_note: candidate.review_note.clone(),
            })
        })
        .collect()
}

fn collect_linked_texture_exclusions(
    inputs: &TranslationTargetInputs<'_>,
) -> Result<Vec<ExcludedJapaneseCandidate>> {
    inputs
        .linked_textures
        .images
        .iter()
        .filter(|image| image.japanese_language_candidate && !image.translation_target)
        .map(|image| {
            let reason = match image.target_decision {
                TextureTargetDecision::ExcludedNoStaticLoadPath => {
                    TranslationExclusionReason::NoStaticLoadPath
                }
                TextureTargetDecision::ExcludedNoRegisteredConsumer => {
                    TranslationExclusionReason::NoRegisteredConsumer
                }
                TextureTargetDecision::ExcludedOtherTitlePromotion => {
                    TranslationExclusionReason::OtherTitlePromotion
                }
                _ => anyhow::bail!(
                    "Japanese linked texture has an invalid exclusion decision: {}:{}",
                    image.image_container_iso_path,
                    image.image_file_record
                ),
            };
            Ok(ExcludedJapaneseCandidate {
                surface: TranslationSurface::LinkedTexture,
                locator: format!(
                    "{}#{}",
                    image.image_container_iso_path, image.image_file_record
                ),
                reason,
                review_note: image.review_note.clone(),
            })
        })
        .collect()
}

fn collect_unlinked_texture_exclusions(
    inputs: &TranslationTargetInputs<'_>,
) -> Result<Vec<ExcludedJapaneseCandidate>> {
    inputs
        .unlinked_textures
        .images
        .iter()
        .filter(|image| image.japanese_language_candidate)
        .map(|image| {
            ensure!(
                !image.translation_target,
                "unlinked Japanese texture unexpectedly became a target"
            );
            Ok(ExcludedJapaneseCandidate {
                surface: TranslationSurface::UnlinkedTexture,
                locator: format!(
                    "{}#{}",
                    inputs.unlinked_textures.container_iso_path, image.image_index
                ),
                reason: TranslationExclusionReason::NoExecutableConsumer,
                review_note: image.review_note.clone(),
            })
        })
        .collect()
}
