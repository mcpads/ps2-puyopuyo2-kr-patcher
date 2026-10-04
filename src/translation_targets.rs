use anyhow::{Result, ensure};

mod collection;
mod model;
mod validation;

use collection::{
    collect_exclusions, collect_executable_targets, collect_linked_texture_targets,
    collect_movie_targets,
};
pub use model::*;
use validation::{validate_evidence_closure, validate_report_identities};

pub fn compose_translation_targets(
    inputs: TranslationTargetInputs<'_>,
) -> Result<TranslationTargetSurvey> {
    validate_report_identities(&inputs)?;
    validate_evidence_closure(&inputs)?;

    let executable_text_targets = collect_executable_targets(&inputs);
    let linked_texture_targets = collect_linked_texture_targets(&inputs);
    let movie_targets = collect_movie_targets(&inputs);
    let excluded_japanese_candidates = collect_exclusions(&inputs)?;

    let unresolved_target_decisions = inputs.executable_strings.summary.unresolved_candidates
        + inputs.storage_gaps.summary.unresolved_candidates
        + inputs.gp_references.summary.unresolved_references
        + inputs
            .computed_references
            .summary
            .unresolved_instruction_words
        + inputs.font_consumers.summary.unresolved_font_consumer_paths
        + inputs.linked_textures.summary.unresolved_images
        + inputs.unlinked_textures.summary.unresolved_images
        + inputs.movies.summary.unresolved_movies;
    ensure!(
        unresolved_target_decisions == 0,
        "translation-target evidence still has unresolved decisions"
    );
    let executable_non_display_exclusions =
        inputs.executable_strings.candidates.len() - executable_text_targets.len();
    let linked_texture_consumer_exclusions = inputs
        .linked_textures
        .summary
        .excluded_japanese_without_consumer;
    let linked_texture_product_scope_exclusions = inputs
        .linked_textures
        .summary
        .excluded_other_title_promotion_images;
    let unlinked_texture_consumer_exclusions =
        inputs.unlinked_textures.summary.japanese_candidate_images;
    let movie_japanese_candidates = inputs
        .movies
        .movies
        .iter()
        .filter(|movie| {
            matches!(
                movie.classification,
                crate::movie_corpus::MovieLanguageClassification::JapaneseOnly
                    | crate::movie_corpus::MovieLanguageClassification::JapaneseEnglishMixed
            )
        })
        .count();
    let movie_product_scope_exclusions = inputs
        .movies
        .movies
        .iter()
        .filter(|movie| {
            matches!(
                movie.classification,
                crate::movie_corpus::MovieLanguageClassification::JapaneseOnly
                    | crate::movie_corpus::MovieLanguageClassification::JapaneseEnglishMixed
            ) && !movie.translation_target
        })
        .count();
    ensure!(
        movie_targets.len() + movie_product_scope_exclusions == movie_japanese_candidates,
        "Japanese PSS target and exclusion partitions do not cover the candidate population"
    );
    let admitted_target_assets =
        executable_text_targets.len() + linked_texture_targets.len() + movie_targets.len();
    let expected_exclusions = executable_non_display_exclusions
        + linked_texture_consumer_exclusions
        + linked_texture_product_scope_exclusions
        + unlinked_texture_consumer_exclusions
        + movie_product_scope_exclusions;
    ensure!(
        excluded_japanese_candidates.len() == expected_exclusions,
        "translation-target exclusion list does not match its partition"
    );

    Ok(TranslationTargetSurvey {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256.to_string(),
        executable_iso_path: inputs.executable_iso_path.to_string(),
        executable_sha256: inputs.executable_sha256.to_string(),
        scope: TranslationTargetScope {
            included: "named and unnamed file-backed executable Japanese candidates with the declared static reference audits, plus consumer-linked D/G/MD images admitted by their explicit product scope",
            target_unit: "one executable string storage or one linked texture image record; PSS files remain physical survey units but enter the product target set only when their explicit product scope is included",
            excluded: "ASCII/English-only storage and images, non-text assets, Japanese-looking binary or font repertoire storage, Japanese images without an admitted executable consumer, other-title promotional graphics and PSS files, runtime-built text, audio speech, and runtime reachability beyond the cited static consumer evidence",
            population_status: "complete for exact-source Japanese-bearing storage assets in the declared executable, texture, and PSS surfaces; every candidate is admitted or excluded with zero unresolved target decisions",
            implementation_boundary: "other-title promotional PSS files stop at exact-source structure, consumer, language, product-scope, and original-byte preservation evidence; they have no translation authoring, encoding, insertion, or runtime-coverage path",
        },
        evidence_checks: TranslationTargetEvidenceChecks {
            executable_storage_gap_candidates: inputs.storage_gaps.summary.storage_gap_candidates,
            new_display_text_from_storage_gaps: inputs.storage_gaps.summary.displayed_japanese_text,
            new_candidates_from_gp_references: inputs
                .gp_references
                .summary
                .newly_referenced_candidates,
            new_candidates_from_computed_references: inputs
                .computed_references
                .summary
                .newly_referenced_candidates,
            conservative_control_flow_boundaries: inputs
                .computed_references
                .summary
                .control_flow_conservative_boundaries,
            unresolved_font_consumer_paths: inputs
                .font_consumers
                .summary
                .unresolved_font_consumer_paths,
            texture_load_tables_without_static_call_path: inputs
                .texture_load_calls
                .unreachable_load_tables
                .len(),
            source_incompatible_texture_calls: inputs
                .texture_load_calls
                .summary
                .source_incompatible_texture_calls,
        },
        summary: TranslationTargetSummary {
            executable_japanese_candidates: inputs
                .executable_strings
                .summary
                .japanese_script_candidates,
            executable_text_targets: executable_text_targets.len(),
            executable_non_display_exclusions,
            linked_texture_japanese_candidates: inputs
                .linked_textures
                .summary
                .japanese_candidate_images,
            linked_texture_targets: linked_texture_targets.len(),
            linked_texture_consumer_exclusions,
            linked_texture_product_scope_exclusions,
            unlinked_texture_japanese_candidates: inputs
                .unlinked_textures
                .summary
                .japanese_candidate_images,
            unlinked_texture_targets: inputs
                .unlinked_textures
                .summary
                .admitted_translation_targets,
            unlinked_texture_consumer_exclusions,
            movie_japanese_candidates,
            movie_targets: movie_targets.len(),
            movie_product_scope_exclusions,
            admitted_target_assets,
            excluded_japanese_candidates: excluded_japanese_candidates.len(),
            unresolved_target_decisions,
        },
        executable_text_targets,
        linked_texture_targets,
        movie_targets,
        excluded_japanese_candidates,
    })
}

#[cfg(test)]
#[path = "translation_targets/translation_target_tests.rs"]
mod translation_target_tests;
