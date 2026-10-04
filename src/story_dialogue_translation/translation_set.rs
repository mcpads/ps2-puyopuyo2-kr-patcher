use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};

use super::{
    STORY_TABLE_COUNT, StoryDialoguePartReference, StoryDialogueTranslationBundle,
    StoryDialogueTranslationManifest, StoryDialogueTranslationPart,
};
use crate::source::{read_json_file, write_json_file};

const SURFACES_PER_PART: usize = 2;
const MANIFEST_FILENAME: &str = "manifest.json";

pub fn story_dialogue_translation_set_files(manifest_path: &Path) -> Result<Vec<PathBuf>> {
    let manifest: StoryDialogueTranslationManifest = read_json_file(manifest_path)?;
    let expected_references = expected_part_references();
    ensure!(
        manifest.schema == 1,
        "unsupported story dialogue manifest schema"
    );
    ensure!(
        manifest.parts == expected_references,
        "story dialogue manifest does not declare the exact consumer-range partition"
    );
    let mut files = Vec::with_capacity(manifest.parts.len() + 1);
    files.push(manifest_path.to_path_buf());
    for reference in &manifest.parts {
        let part_path = resolve_part_path(manifest_path, &reference.relative_path)?;
        let part: StoryDialogueTranslationPart = read_json_file(&part_path)?;
        validate_part(reference, &part, &part_path)?;
        files.push(part_path);
    }
    Ok(files)
}

pub fn load_story_dialogue_translation_set(
    manifest_path: &Path,
) -> Result<StoryDialogueTranslationBundle> {
    let manifest: StoryDialogueTranslationManifest = read_json_file(manifest_path)?;
    let expected_references = expected_part_references();
    ensure!(
        manifest.schema == 1,
        "unsupported story dialogue manifest schema"
    );
    ensure!(
        manifest.parts == expected_references,
        "story dialogue manifest does not declare the exact consumer-range partition"
    );

    let mut entries = Vec::with_capacity(STORY_TABLE_COUNT);
    for reference in &manifest.parts {
        let part_path = resolve_part_path(manifest_path, &reference.relative_path)?;
        let part: StoryDialogueTranslationPart = read_json_file(&part_path)?;
        validate_part(reference, &part, &part_path)?;
        entries.extend(part.entries);
    }

    Ok(StoryDialogueTranslationBundle {
        schema: manifest.schema,
        source: manifest.source,
        scope: manifest.scope,
        population: manifest.population,
        entries,
    })
}

pub fn write_new_story_dialogue_translation_set(
    manifest_path: &Path,
    bundle: &StoryDialogueTranslationBundle,
) -> Result<()> {
    ensure!(
        manifest_path.file_name().and_then(|name| name.to_str()) == Some(MANIFEST_FILENAME),
        "story dialogue set output must end in {MANIFEST_FILENAME}"
    );
    let output_root = manifest_path
        .parent()
        .context("story dialogue manifest has no parent directory")?;
    ensure!(
        !output_root.exists(),
        "refusing to overwrite story dialogue translation set {}",
        output_root.display()
    );
    let output_parent = output_root
        .parent()
        .context("story dialogue translation set has no parent directory")?;
    std::fs::create_dir_all(output_parent).with_context(|| {
        format!(
            "failed to create story dialogue output parent {}",
            output_parent.display()
        )
    })?;

    let (manifest, parts) = partition_bundle(bundle)?;
    let temporary = tempfile::Builder::new()
        .prefix(".story-dialogue-")
        .tempdir_in(output_parent)
        .with_context(|| {
            format!(
                "failed to create temporary story dialogue set in {}",
                output_parent.display()
            )
        })?;
    for (reference, part) in parts {
        let part_path = temporary.path().join(&reference.relative_path);
        write_json_file(&part_path, &part)?;
    }
    write_json_file(&temporary.path().join(MANIFEST_FILENAME), &manifest)?;
    let temporary_path = temporary.keep();
    std::fs::rename(&temporary_path, output_root).with_context(|| {
        format!(
            "failed to commit story dialogue translation set {}",
            output_root.display()
        )
    })?;
    Ok(())
}

fn partition_bundle(
    bundle: &StoryDialogueTranslationBundle,
) -> Result<(
    StoryDialogueTranslationManifest,
    Vec<(StoryDialoguePartReference, StoryDialogueTranslationPart)>,
)> {
    ensure!(
        bundle.schema == 1,
        "unsupported story dialogue translation schema"
    );
    ensure!(
        bundle.entries.len() == STORY_TABLE_COUNT,
        "story dialogue bundle must contain exactly {STORY_TABLE_COUNT} surfaces before partitioning"
    );
    for (sequence, entry) in bundle.entries.iter().enumerate() {
        ensure!(
            entry.source.consumer_sequence == sequence,
            "story dialogue bundle is not in exact consumer sequence order at {sequence:#04x}"
        );
    }

    let references = expected_part_references();
    let parts = references
        .iter()
        .map(|reference| {
            let entries = bundle.entries
                [reference.first_consumer_sequence..=reference.last_consumer_sequence]
                .to_vec();
            (
                reference.clone(),
                StoryDialogueTranslationPart {
                    schema: 1,
                    part_id: reference.part_id.clone(),
                    entries,
                },
            )
        })
        .collect();
    let manifest = StoryDialogueTranslationManifest {
        schema: bundle.schema,
        source: bundle.source.clone(),
        scope: bundle.scope.clone(),
        population: bundle.population.clone(),
        parts: references,
    };
    Ok((manifest, parts))
}

fn expected_part_references() -> Vec<StoryDialoguePartReference> {
    (0..STORY_TABLE_COUNT)
        .step_by(SURFACES_PER_PART)
        .map(|first| {
            let last = first + SURFACES_PER_PART - 1;
            let part_id = format!("demo-{first:02X}-{last:02X}");
            StoryDialoguePartReference {
                relative_path: format!("parts/{part_id}.json"),
                part_id,
                first_consumer_sequence: first,
                last_consumer_sequence: last,
            }
        })
        .collect()
}

fn resolve_part_path(manifest_path: &Path, relative_path: &str) -> Result<PathBuf> {
    let relative_path = Path::new(relative_path);
    ensure!(
        !relative_path.is_absolute()
            && relative_path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "story dialogue part path is not a safe relative path: {}",
        relative_path.display()
    );
    let root = manifest_path
        .parent()
        .context("story dialogue manifest has no parent directory")?;
    Ok(root.join(relative_path))
}

fn validate_part(
    reference: &StoryDialoguePartReference,
    part: &StoryDialogueTranslationPart,
    part_path: &Path,
) -> Result<()> {
    ensure!(
        part.schema == 1,
        "unsupported story dialogue part schema: {}",
        part_path.display()
    );
    ensure!(
        part.part_id == reference.part_id,
        "story dialogue part identity differs from the manifest: {}",
        part_path.display()
    );
    ensure!(
        part.entries.len() == SURFACES_PER_PART,
        "story dialogue part does not contain exactly {SURFACES_PER_PART} surfaces: {}",
        part_path.display()
    );
    for (entry, expected_sequence) in part
        .entries
        .iter()
        .zip(reference.first_consumer_sequence..=reference.last_consumer_sequence)
    {
        ensure!(
            entry.source.consumer_sequence == expected_sequence,
            "story dialogue part contains a surface outside its consumer range: {}",
            part_path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "translation_set_tests.rs"]
mod translation_set_tests;
