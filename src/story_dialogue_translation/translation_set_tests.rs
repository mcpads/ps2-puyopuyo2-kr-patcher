use tempfile::tempdir;

use super::*;
use crate::graphics_translation::GraphicsTranslationBundleSource;
use crate::story_dialogue_translation::{
    StoryDialogueAuthoring, StoryDialogueStatus, StoryDialogueSurfaceSource,
    StoryDialogueTranslationEntry, StoryDialogueTranslationPopulation,
    StoryDialogueTranslationScope,
};
use crate::texture_language_survey::TextureLanguageClassification;

fn translation_bundle() -> StoryDialogueTranslationBundle {
    StoryDialogueTranslationBundle {
        schema: 1,
        source: GraphicsTranslationBundleSource {
            source_bin_sha256: "a".repeat(64),
            executable_iso_path: "/SLPM_624.00;1".to_string(),
            executable_sha256: "b".repeat(64),
        },
        scope: StoryDialogueTranslationScope {
            target: "Japanese dialogue".to_string(),
            preserve: "English pixels".to_string(),
            reference_policy: "source first".to_string(),
            build_policy: "review before release".to_string(),
            release_candidate: false,
        },
        population: StoryDialogueTranslationPopulation {
            consumer_tables: STORY_TABLE_COUNT,
            dialogue_surfaces: STORY_TABLE_COUNT,
            unresolved_tables: 0,
        },
        entries: (0..STORY_TABLE_COUNT).map(translation_entry).collect(),
    }
}

fn translation_entry(sequence: usize) -> StoryDialogueTranslationEntry {
    StoryDialogueTranslationEntry {
        source: StoryDialogueSurfaceSource {
            story_surface_id: format!("story-dialogue/demo-{sequence:02X}"),
            consumer_table: format!("DEMOTexTbl{sequence:02X}"),
            consumer_sequence: sequence,
            source_surface_id: format!("linked-texture/image-md/{sequence:06}"),
            container_iso_path: "/IMAGE_MD.BIN;1".to_string(),
            image_file_record: sequence,
            width: 512,
            height: 256,
            linear_indices_sha256: format!("{sequence:064x}"),
            classification: TextureLanguageClassification::JapaneseOnly,
            requires_english_pixel_preservation: false,
        },
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
    }
}

#[test]
fn split_set_round_trip_preserves_every_surface_and_authoring_field() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("story-dialogue/manifest.json");
    let original = translation_bundle();

    write_new_story_dialogue_translation_set(&manifest_path, &original).unwrap();
    let loaded = load_story_dialogue_translation_set(&manifest_path).unwrap();
    let files = story_dialogue_translation_set_files(&manifest_path).unwrap();

    assert_eq!(loaded, original);
    assert_eq!(files.len(), STORY_TABLE_COUNT / SURFACES_PER_PART + 1);
    assert_eq!(files[0], manifest_path);
}

#[test]
fn loader_rejects_a_surface_stored_outside_its_declared_consumer_range() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("story-dialogue/manifest.json");
    write_new_story_dialogue_translation_set(&manifest_path, &translation_bundle()).unwrap();
    let part_path = manifest_path
        .parent()
        .unwrap()
        .join("parts/demo-00-01.json");
    let mut part: StoryDialogueTranslationPart = read_json_file(&part_path).unwrap();
    part.entries.swap(0, 1);
    write_json_file(&part_path, &part).unwrap();

    let error = load_story_dialogue_translation_set(&manifest_path).unwrap_err();
    assert!(error.to_string().contains("outside its consumer range"));
}

#[test]
fn loader_rejects_manifest_reordering_instead_of_using_file_order() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("story-dialogue/manifest.json");
    write_new_story_dialogue_translation_set(&manifest_path, &translation_bundle()).unwrap();
    let mut manifest: StoryDialogueTranslationManifest = read_json_file(&manifest_path).unwrap();
    manifest.parts.swap(0, 1);
    write_json_file(&manifest_path, &manifest).unwrap();

    let error = load_story_dialogue_translation_set(&manifest_path).unwrap_err();
    assert!(error.to_string().contains("exact consumer-range partition"));
}

#[test]
fn initialization_refuses_to_overwrite_an_existing_translation_set() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("story-dialogue/manifest.json");
    let bundle = translation_bundle();
    write_new_story_dialogue_translation_set(&manifest_path, &bundle).unwrap();

    let error = write_new_story_dialogue_translation_set(&manifest_path, &bundle).unwrap_err();
    assert!(error.to_string().contains("refusing to overwrite"));
}
