use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use super::{
    StoryDialogueAuthoring, StoryDialogueStatus, StoryDialogueTranslationBundle,
    StoryDialogueTranslationValidation, compose_story_dialogue_translation_bundle,
};
use crate::graphics_translation::GraphicsTranslationBundle;

pub fn validate_story_dialogue_translation_bundle(
    bundle: &StoryDialogueTranslationBundle,
    graphics: &GraphicsTranslationBundle,
) -> Result<StoryDialogueTranslationValidation> {
    let expected = compose_story_dialogue_translation_bundle(graphics)?;
    ensure!(
        bundle.schema == 1,
        "unsupported story dialogue translation schema"
    );
    ensure!(
        bundle.source == expected.source
            && bundle.scope == expected.scope
            && bundle.population == expected.population,
        "story dialogue translation header differs from the exact graphics baseline"
    );
    ensure!(
        bundle.entries.len() == expected.entries.len(),
        "story dialogue translation bundle does not cover the exact surface population"
    );
    let expected_sources = expected
        .entries
        .into_iter()
        .map(|entry| (entry.source.story_surface_id.clone(), entry.source))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut previous_sequence = None;
    let mut validation = StoryDialogueTranslationValidation {
        dialogue_surfaces: bundle.entries.len(),
        needs_transcription: 0,
        needs_review: 0,
        needs_human_review: 0,
        complete: 0,
        release_candidate_eligible: 0,
        transcribed_segments: 0,
        source_bound_layout_lines: 0,
        source_characters: 0,
        korean_characters: 0,
        sibling_reference_links: 0,
    };

    for entry in &bundle.entries {
        let source = &entry.source;
        ensure!(
            previous_sequence.is_none_or(|previous| previous < source.consumer_sequence),
            "story dialogue entries are not in consumer sequence order"
        );
        previous_sequence = Some(source.consumer_sequence);
        ensure!(
            seen.insert(source.story_surface_id.as_str()),
            "duplicate story dialogue surface: {}",
            source.story_surface_id
        );
        let expected_source = expected_sources
            .get(&source.story_surface_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "unexpected story dialogue surface: {}",
                    source.story_surface_id
                )
            })?;
        ensure!(
            source == expected_source,
            "protected story source fields differ: {}",
            source.story_surface_id
        );
        validate_authoring(&source.story_surface_id, &entry.authoring)?;
        validation.transcribed_segments += entry.authoring.segments.len();
        validation.source_bound_layout_lines += entry
            .authoring
            .segments
            .iter()
            .map(|segment| segment.korean_text.split('\n').count())
            .sum::<usize>();
        validation.source_characters += entry
            .authoring
            .segments
            .iter()
            .map(|segment| segment.source_text.chars().filter(|c| *c != '\n').count())
            .sum::<usize>();
        validation.korean_characters += entry
            .authoring
            .segments
            .iter()
            .map(|segment| segment.korean_text.chars().filter(|c| *c != '\n').count())
            .sum::<usize>();
        validation.sibling_reference_links += entry
            .authoring
            .segments
            .iter()
            .map(|segment| segment.sibling_references.len())
            .sum::<usize>();
        match entry.authoring.status {
            StoryDialogueStatus::NeedsTranscription => validation.needs_transcription += 1,
            StoryDialogueStatus::NeedsReview => validation.needs_review += 1,
            StoryDialogueStatus::NeedsHumanReview => validation.needs_human_review += 1,
            StoryDialogueStatus::Complete => validation.complete += 1,
        }
    }
    ensure!(
        seen.len() == expected_sources.len(),
        "story dialogue translation bundle is missing an exact-source surface"
    );
    Ok(validation)
}

fn validate_authoring(surface_id: &str, authoring: &StoryDialogueAuthoring) -> Result<()> {
    let has_scene_identity = authoring
        .scene_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        && authoring
            .scene_variant
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    match authoring.status {
        StoryDialogueStatus::NeedsTranscription => ensure!(
            !has_scene_identity
                && authoring.segments.is_empty()
                && authoring.transcription_review.is_none()
                && authoring.human_approval.is_none()
                && authoring.independent_review.is_none(),
            "untranscribed story surface contains later-stage data: {surface_id}"
        ),
        StoryDialogueStatus::NeedsReview => ensure!(
            has_scene_identity
                && !authoring.segments.is_empty()
                && authoring.transcription_review.is_none()
                && authoring.human_approval.is_none()
                && authoring.independent_review.is_none(),
            "review-pending story surface lacks a draft or contains decision evidence: {surface_id}"
        ),
        StoryDialogueStatus::NeedsHumanReview => ensure!(
            has_scene_identity
                && !authoring.segments.is_empty()
                && authoring.transcription_review.is_some()
                && authoring.human_approval.is_none()
                && authoring.independent_review.is_none(),
            "human-review story surface lacks verified transcription or already contains approval: {surface_id}"
        ),
        StoryDialogueStatus::Complete => anyhow::bail!(
            "complete story status is unavailable until human approval, protected-pixel insertion, runtime evidence, and release-candidate eligibility are represented: {surface_id}"
        ),
    }

    let mut segment_ids = BTreeSet::new();
    for segment in &authoring.segments {
        ensure!(
            segment_ids.insert(segment.segment_id.as_str()),
            "duplicate story segment in {surface_id}: {}",
            segment.segment_id
        );
        ensure!(
            !segment.segment_id.trim().is_empty()
                && !segment.speaker_id.trim().is_empty()
                && !segment.source_text.trim().is_empty()
                && !segment.korean_text.trim().is_empty(),
            "story segment lacks identity, transcription, or Korean draft: {surface_id}"
        );
        let source_line_slots = segment.source_text.split('\n').count();
        let korean_layout_lines = segment.korean_text.split('\n').count();
        ensure!(
            source_line_slots == korean_layout_lines,
            "story Korean layout line slot count differs from its source consumer slots: {surface_id}/{} ({source_line_slots} source, {korean_layout_lines} Korean)",
            segment.segment_id
        );
        ensure!(
            segment.source_text.chars().any(is_japanese),
            "story source transcription has no Japanese script: {surface_id}/{}",
            segment.segment_id
        );
        ensure!(
            !segment.korean_text.chars().any(is_japanese),
            "story Korean draft still contains Japanese script: {surface_id}/{}",
            segment.segment_id
        );
        let mut sibling_ids = BTreeSet::new();
        for reference in &segment.sibling_references {
            ensure!(
                reference.stable_id.starts_with("story_")
                    && !reference.note.trim().is_empty()
                    && sibling_ids.insert(reference.stable_id.as_str()),
                "story sibling reference is invalid or duplicated: {surface_id}/{}",
                segment.segment_id
            );
        }
    }
    Ok(())
}

fn is_japanese(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' | '\u{3400}'..='\u{9fff}'
    )
}

#[cfg(test)]
pub(super) fn validate_authoring_for_test(
    surface_id: &str,
    authoring: &StoryDialogueAuthoring,
) -> Result<()> {
    validate_authoring(surface_id, authoring)
}
