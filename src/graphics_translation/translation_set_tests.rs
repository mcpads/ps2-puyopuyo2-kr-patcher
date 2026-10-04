use tempfile::tempdir;

use super::*;
use crate::graphics_translation::{
    GraphicsTranslationInputs, GraphicsTranslationStatus, compose_graphics_translation_bundle,
};
use crate::source::{read_json_file, write_json_file};
use crate::texture_language_survey::TextureLanguageClassification;
use crate::translation_targets::LinkedTextureTarget;

fn target(container: &str, record: usize) -> LinkedTextureTarget {
    LinkedTextureTarget {
        container_iso_path: container.to_string(),
        image_file_record: record,
        width: 640,
        height: 512,
        linear_indices_sha256: format!("{record:064x}"),
        classification: TextureLanguageClassification::JapaneseOnly,
        requires_english_pixel_preservation: false,
        owner_tables: vec!["ConsumerTable".to_string()],
    }
}

fn translation_bundle() -> GraphicsTranslationBundle {
    let targets = vec![
        target("/IMAGE_D.BIN;1", 19),
        target("/IMAGE_G.BIN;1", 39),
        target("/IMAGE_MD.BIN;1", 300),
    ];
    compose_graphics_translation_bundle(GraphicsTranslationInputs {
        source_bin_sha256: &"a".repeat(64),
        executable_iso_path: "/SLPM_624.00;1",
        executable_sha256: &"b".repeat(64),
        linked_texture_targets: &targets,
    })
    .unwrap()
}

#[test]
fn split_set_round_trip_preserves_every_surface_and_authoring_field() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("graphics-text/manifest.json");
    let original = translation_bundle();

    write_new_graphics_translation_set(&manifest_path, &original).unwrap();
    let loaded = load_graphics_translation_set(&manifest_path).unwrap();
    let files = graphics_translation_set_files(&manifest_path).unwrap();

    assert_eq!(loaded, original);
    assert_eq!(files.len(), 4);
    assert_eq!(files[0], manifest_path);
}

#[test]
fn initialization_keeps_unknown_semantic_responsibilities_isolated() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("graphics-text/manifest.json");
    write_new_graphics_translation_set(&manifest_path, &translation_bundle()).unwrap();
    let manifest: GraphicsTranslationManifest = read_json_file(&manifest_path).unwrap();

    assert_eq!(manifest.parts.len(), 3);
    assert!(manifest.parts.iter().all(|part| part.entry_count == 1));
    assert!(
        manifest
            .parts
            .iter()
            .all(|part| part.relative_path.starts_with("parts/pending/"))
    );
}

#[test]
fn saving_updates_entries_without_collapsing_declared_responsibilities() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("graphics-text/manifest.json");
    write_new_graphics_translation_set(&manifest_path, &translation_bundle()).unwrap();
    let manifest_before: GraphicsTranslationManifest = read_json_file(&manifest_path).unwrap();
    let mut updated = load_graphics_translation_set(&manifest_path).unwrap();
    updated.entries[1].authoring.status = GraphicsTranslationStatus::NeedsRegionInventory;
    updated.entries[1].authoring.semantic_asset_id = Some("battle/tea-label".to_string());
    updated.entries[1].authoring.semantic_role = Some("character_reaction".to_string());
    updated.entries[1].authoring.continuity_group = Some("tea_label".to_string());

    write_graphics_translation_set(&manifest_path, &updated).unwrap();

    let manifest_after: GraphicsTranslationManifest = read_json_file(&manifest_path).unwrap();
    let reloaded = load_graphics_translation_set(&manifest_path).unwrap();
    assert_eq!(manifest_after.parts, manifest_before.parts);
    assert_eq!(reloaded, updated);
}

#[test]
fn loading_is_independent_of_manifest_part_order() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("graphics-text/manifest.json");
    let original = translation_bundle();
    write_new_graphics_translation_set(&manifest_path, &original).unwrap();
    let mut manifest: GraphicsTranslationManifest = read_json_file(&manifest_path).unwrap();
    manifest.parts.reverse();
    write_json_file(&manifest_path, &manifest).unwrap();

    let loaded = load_graphics_translation_set(&manifest_path).unwrap();

    assert_eq!(loaded, original);
}

#[test]
fn loader_rejects_duplicate_surface_ownership() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("graphics-text/manifest.json");
    write_new_graphics_translation_set(&manifest_path, &translation_bundle()).unwrap();
    let manifest: GraphicsTranslationManifest = read_json_file(&manifest_path).unwrap();
    let first_path = manifest_path
        .parent()
        .unwrap()
        .join(&manifest.parts[0].relative_path);
    let second_path = manifest_path
        .parent()
        .unwrap()
        .join(&manifest.parts[1].relative_path);
    let first: GraphicsTranslationPart = read_json_file(&first_path).unwrap();
    let mut second: GraphicsTranslationPart = read_json_file(&second_path).unwrap();
    second.entries[0] = first.entries[0].clone();
    write_json_file(&second_path, &second).unwrap();

    let error = load_graphics_translation_set(&manifest_path).unwrap_err();
    assert!(error.to_string().contains("owned by more than one part"));
}

#[test]
fn loader_rejects_unsafe_part_paths() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("graphics-text/manifest.json");
    write_new_graphics_translation_set(&manifest_path, &translation_bundle()).unwrap();
    let mut manifest: GraphicsTranslationManifest = read_json_file(&manifest_path).unwrap();
    manifest.parts[0].relative_path = "../outside.json".to_string();
    write_json_file(&manifest_path, &manifest).unwrap();

    let error = load_graphics_translation_set(&manifest_path).unwrap_err();
    assert!(error.to_string().contains("not a safe relative path"));
}

#[test]
fn initialization_refuses_to_overwrite_an_existing_translation_set() {
    let directory = tempdir().unwrap();
    let manifest_path = directory.path().join("graphics-text/manifest.json");
    let bundle = translation_bundle();
    write_new_graphics_translation_set(&manifest_path, &bundle).unwrap();

    let error = write_new_graphics_translation_set(&manifest_path, &bundle).unwrap_err();
    assert!(error.to_string().contains("refusing to overwrite"));
}
