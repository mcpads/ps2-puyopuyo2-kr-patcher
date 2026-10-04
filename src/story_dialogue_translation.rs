use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::graphics_translation::{GraphicsTranslationBundle, GraphicsTranslationSource};
use crate::texture_language_survey::TextureLanguageClassification;

mod consumer;
mod font;
mod layout;
mod model;
mod protected_english;
mod replacement;
mod sequence;
mod translation_set;
mod validation;

pub use consumer::{
    StoryDialogueTextureCellConsumerValidation, validate_story_dialogue_texture_cell_consumer,
};
pub use layout::{
    StoryDialogueRenderedLayoutValidation, StoryDialogueRowOverflow,
    measure_story_dialogue_rendered_layout,
};
pub use model::*;
pub(crate) use replacement::plan_story_dialogue_translation;
pub use replacement::{
    StoryDialogueReplacementInputs, StoryDialogueReplacementReport,
    StoryDialogueSurfaceReplacement, StoryDialogueTranslationBuildInputs,
    StoryDialogueTranslationBuildReport, build_story_dialogue_translation_development,
    render_story_dialogue_replacements,
};
pub use sequence::{
    StoryDialogueSequenceLayoutValidation, validate_story_dialogue_sequence_layout,
};
pub use translation_set::{
    load_story_dialogue_translation_set, story_dialogue_translation_set_files,
    write_new_story_dialogue_translation_set,
};
pub use validation::validate_story_dialogue_translation_bundle;

const IMAGE_MD_PATH: &str = "/IMAGE_MD.BIN;1";
const FIRST_STORY_IMAGE_RECORD: usize = 72;
const NON_DIALOGUE_IMAGE_RECORD: usize = 99;
const LAST_STORY_IMAGE_RECORD: usize = 146;
pub(super) const STORY_TABLE_COUNT: usize = 0x4a;

pub fn compose_story_dialogue_translation_bundle(
    graphics: &GraphicsTranslationBundle,
) -> Result<StoryDialogueTranslationBundle> {
    let mut surfaces = BTreeMap::<usize, StoryDialogueSurfaceSource>::new();
    for entry in &graphics.entries {
        let source = &entry.source;
        if source.container_iso_path != IMAGE_MD_PATH
            || !(FIRST_STORY_IMAGE_RECORD..=LAST_STORY_IMAGE_RECORD)
                .contains(&source.image_file_record)
            || source.image_file_record == NON_DIALOGUE_IMAGE_RECORD
        {
            continue;
        }
        ensure!(
            matches!(
                source.classification,
                TextureLanguageClassification::JapaneseOnly
                    | TextureLanguageClassification::JapaneseEnglishMixed
            ),
            "story dialogue surface is not classified as Japanese-bearing: {}",
            source.source_surface_id
        );
        ensure!(
            source.owner_tables.len() == 1,
            "story dialogue surface does not have exactly one consumer table: {}",
            source.source_surface_id
        );
        let consumer_table = source.owner_tables[0].as_str();
        let sequence = parse_story_table_sequence(consumer_table)?;
        let story_source = source_from_graphics(sequence, consumer_table, source);
        ensure!(
            surfaces.insert(sequence, story_source).is_none(),
            "duplicate story dialogue consumer sequence: {sequence}"
        );
    }
    ensure!(
        surfaces.len() == STORY_TABLE_COUNT,
        "story dialogue surface population is not {STORY_TABLE_COUNT}"
    );
    for sequence in 0..STORY_TABLE_COUNT {
        ensure!(
            surfaces.contains_key(&sequence),
            "story dialogue consumer sequence is missing: {sequence:#04x}"
        );
    }

    let entries = surfaces
        .into_values()
        .map(|source| StoryDialogueTranslationEntry {
            source,
            authoring: StoryDialogueAuthoring {
                status: StoryDialogueStatus::NeedsTranscription,
                scene_id: None,
                scene_variant: None,
                segments: Vec::new(),
                transcription_notes: Vec::new(),
                transcription_review: None,
                human_approval: None,
                independent_review: None,
            },
        })
        .collect::<Vec<_>>();

    Ok(StoryDialogueTranslationBundle {
        schema: 1,
        source: graphics.source.clone(),
        scope: StoryDialogueTranslationScope {
            target: "Japanese dialogue pixels baked into the 74 IMAGE_MD surfaces consumed by DEMOTexTbl00 through DEMOTexTbl49".to_string(),
            preserve: "English pixels and all non-dialogue artwork; source surface dimensions, palette-index identity, and consumer order".to_string(),
            reference_policy: "PS2 pixels and DEMOTexTbl consumer order are authoritative; SNES stable IDs are read-only exact or near/expanded translation references, never imported approval".to_string(),
            build_policy: "transcription, direct Korean drafting, human language approval, protected-pixel editing, insertion, and runtime evidence are separate gates; pending review does not block explicitly non-distribution development work".to_string(),
            release_candidate: false,
        },
        population: StoryDialogueTranslationPopulation {
            consumer_tables: STORY_TABLE_COUNT,
            dialogue_surfaces: entries.len(),
            unresolved_tables: 0,
        },
        entries,
    })
}

fn parse_story_table_sequence(table: &str) -> Result<usize> {
    let suffix = table
        .strip_prefix("DEMOTexTbl")
        .with_context(|| format!("unexpected story dialogue consumer table: {table}"))?;
    ensure!(
        suffix.len() == 2
            && suffix
                .chars()
                .all(|character| character.is_ascii_hexdigit()),
        "story dialogue consumer table does not end in two hex digits: {table}"
    );
    let sequence = usize::from_str_radix(suffix, 16)
        .with_context(|| format!("invalid story dialogue table index: {table}"))?;
    ensure!(
        sequence < STORY_TABLE_COUNT,
        "story dialogue table index is outside DEMOTexTbl00..49: {table}"
    );
    Ok(sequence)
}

fn source_from_graphics(
    sequence: usize,
    consumer_table: &str,
    source: &GraphicsTranslationSource,
) -> StoryDialogueSurfaceSource {
    StoryDialogueSurfaceSource {
        story_surface_id: format!("story-dialogue/demo-{sequence:02X}"),
        consumer_table: consumer_table.to_string(),
        consumer_sequence: sequence,
        source_surface_id: source.source_surface_id.clone(),
        container_iso_path: source.container_iso_path.clone(),
        image_file_record: source.image_file_record,
        width: source.width,
        height: source.height,
        linear_indices_sha256: source.linear_indices_sha256.clone(),
        classification: source.classification,
        requires_english_pixel_preservation: source.requires_english_pixel_preservation,
    }
}

#[cfg(test)]
#[path = "story_dialogue_translation/consumer_tests.rs"]
mod consumer_tests;

#[cfg(test)]
#[path = "story_dialogue_translation/sequence_tests.rs"]
mod sequence_tests;

#[cfg(test)]
#[path = "story_dialogue_translation/story_dialogue_translation_tests.rs"]
mod story_dialogue_translation_tests;
