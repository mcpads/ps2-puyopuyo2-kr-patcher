use anyhow::{Result, ensure};

use super::TranslationTargetInputs;
use crate::product_scope::ProductScope;

pub(super) fn validate_report_identities(inputs: &TranslationTargetInputs<'_>) -> Result<()> {
    let identities = [
        (
            inputs.executable_strings.source_bin_sha256.as_str(),
            inputs.executable_strings.executable_iso_path.as_str(),
            inputs.executable_strings.executable_sha256.as_str(),
        ),
        (
            inputs.storage_gaps.source_bin_sha256.as_str(),
            inputs.storage_gaps.executable_iso_path.as_str(),
            inputs.storage_gaps.executable_sha256.as_str(),
        ),
        (
            inputs.gp_references.source_bin_sha256.as_str(),
            inputs.gp_references.executable_iso_path.as_str(),
            inputs.gp_references.executable_sha256.as_str(),
        ),
        (
            inputs.computed_references.source_bin_sha256.as_str(),
            inputs.computed_references.executable_iso_path.as_str(),
            inputs.computed_references.executable_sha256.as_str(),
        ),
        (
            inputs.font_consumers.source_bin_sha256.as_str(),
            inputs.font_consumers.executable_iso_path.as_str(),
            inputs.font_consumers.executable_sha256.as_str(),
        ),
        (
            inputs.texture_load_calls.source_bin_sha256.as_str(),
            inputs.texture_load_calls.executable_iso_path.as_str(),
            inputs.texture_load_calls.executable_sha256.as_str(),
        ),
        (
            inputs.linked_textures.source_bin_sha256.as_str(),
            inputs.linked_textures.executable_iso_path.as_str(),
            inputs.linked_textures.executable_sha256.as_str(),
        ),
        (
            inputs.unlinked_textures.source_bin_sha256.as_str(),
            inputs.unlinked_textures.executable_iso_path.as_str(),
            inputs.unlinked_textures.executable_sha256.as_str(),
        ),
        (
            inputs.movies.source_bin_sha256.as_str(),
            inputs.movies.executable_iso_path.as_str(),
            inputs.movies.executable_sha256.as_str(),
        ),
    ];
    ensure!(
        identities.iter().all(|identity| {
            identity
                == &(
                    inputs.source_bin_sha256,
                    inputs.executable_iso_path,
                    inputs.executable_sha256,
                )
        }),
        "translation-target inputs do not share one exact source identity"
    );
    Ok(())
}

pub(super) fn validate_evidence_closure(inputs: &TranslationTargetInputs<'_>) -> Result<()> {
    ensure!(
        inputs.storage_gaps.summary.displayed_japanese_text == 0,
        "unnamed executable storage adds displayed Japanese text"
    );
    ensure!(
        inputs.gp_references.summary.newly_referenced_candidates == 0
            && inputs
                .computed_references
                .summary
                .newly_referenced_candidates
                == 0,
        "computed executable references add an unclassified candidate"
    );
    ensure!(
        inputs.font_consumers.summary.unresolved_font_consumer_paths == 0,
        "a Shift-JIS font consumer path remains unresolved"
    );
    ensure!(
        inputs.texture_load_calls.unreachable_load_tables.is_empty(),
        "a texture load table has no resolved static call path"
    );
    validate_texture_target_partitions(
        inputs.linked_textures.summary.japanese_candidate_images,
        inputs.linked_textures.summary.admitted_translation_targets,
        inputs
            .linked_textures
            .summary
            .excluded_japanese_without_consumer,
        inputs
            .linked_textures
            .summary
            .excluded_other_title_promotion_images,
        inputs.unlinked_textures.summary.japanese_candidate_images,
        inputs
            .unlinked_textures
            .summary
            .admitted_translation_targets,
    )?;
    ensure!(
        inputs.movies.summary.included_movies
            + inputs.movies.summary.excluded_other_title_promotion_movies
            == inputs.movies.summary.movies,
        "movie product scope does not partition the physical corpus"
    );
    ensure!(
        inputs
            .movies
            .movies
            .iter()
            .filter(|movie| movie.translation_target)
            .count()
            == inputs.movies.summary.japanese_target_movies,
        "movie target summary differs from the surveyed entries"
    );
    ensure!(
        inputs.movies.movies.iter().all(|movie| {
            !movie.translation_target || movie.product_scope == ProductScope::Included
        }),
        "a product-excluded movie was admitted as a translation target"
    );
    Ok(())
}

pub(super) fn validate_texture_target_partitions(
    linked_candidates: usize,
    linked_targets: usize,
    linked_consumer_exclusions: usize,
    linked_product_scope_exclusions: usize,
    unlinked_candidates: usize,
    unlinked_targets: usize,
) -> Result<()> {
    ensure!(
        linked_targets + linked_consumer_exclusions + linked_product_scope_exclusions
            == linked_candidates,
        "linked Japanese textures are not fully admitted or excluded"
    );
    ensure!(
        unlinked_targets == 0,
        "an unlinked texture was admitted without an executable consumer"
    );
    ensure!(
        unlinked_candidates >= unlinked_targets,
        "unlinked texture targets exceed Japanese candidates"
    );
    Ok(())
}
