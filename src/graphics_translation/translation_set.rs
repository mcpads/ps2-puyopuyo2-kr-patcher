use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};

use super::{
    GraphicsTranslationBundle, GraphicsTranslationManifest, GraphicsTranslationPart,
    GraphicsTranslationPartReference,
};
use crate::source::{read_json_file, write_json_file};

const MANIFEST_FILENAME: &str = "manifest.json";

pub fn graphics_translation_set_files(manifest_path: &Path) -> Result<Vec<PathBuf>> {
    let manifest: GraphicsTranslationManifest = read_json_file(manifest_path)?;
    load_parts(manifest_path, &manifest)?;
    let mut part_paths = manifest
        .parts
        .iter()
        .map(|reference| resolve_part_path(manifest_path, &reference.relative_path))
        .collect::<Result<Vec<_>>>()?;
    part_paths.sort();
    let mut files = Vec::with_capacity(part_paths.len() + 1);
    files.push(manifest_path.to_path_buf());
    files.extend(part_paths);
    Ok(files)
}

pub fn load_graphics_translation_set(manifest_path: &Path) -> Result<GraphicsTranslationBundle> {
    let manifest: GraphicsTranslationManifest = read_json_file(manifest_path)?;
    let loaded = load_parts(manifest_path, &manifest)?;
    let mut entries = loaded
        .into_iter()
        .flat_map(|(_, part)| part.entries)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.source
            .source_surface_id
            .cmp(&right.source.source_surface_id)
    });

    Ok(GraphicsTranslationBundle {
        schema: manifest.schema,
        source: manifest.source,
        scope: manifest.scope,
        population: manifest.population,
        entries,
    })
}

pub fn write_graphics_translation_set(
    manifest_path: &Path,
    bundle: &GraphicsTranslationBundle,
) -> Result<()> {
    validate_manifest_filename(manifest_path)?;
    ensure!(
        bundle.schema == 1,
        "unsupported graphics translation schema"
    );
    ensure!(
        bundle.entries.len() == bundle.population.linked_texture_surfaces,
        "graphics translation bundle population differs from its entries"
    );
    let existing_manifest: GraphicsTranslationManifest = read_json_file(manifest_path)?;
    let existing_parts = load_parts(manifest_path, &existing_manifest)?;
    ensure!(
        bundle.source == existing_manifest.source,
        "graphics translation source identity cannot change while saving a translation set"
    );
    ensure!(
        bundle.population == existing_manifest.population,
        "graphics translation population cannot change while saving a translation set"
    );
    let mut entries_by_id = bundle
        .entries
        .iter()
        .cloned()
        .map(|entry| (entry.source.source_surface_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        entries_by_id.len() == bundle.entries.len(),
        "graphics translation bundle contains duplicate source surface identities"
    );

    let mut updated_parts = Vec::with_capacity(existing_parts.len());
    for (reference, existing_part) in existing_parts {
        let mut entries = Vec::with_capacity(existing_part.entries.len());
        for existing_entry in existing_part.entries {
            let source_surface_id = &existing_entry.source.source_surface_id;
            let entry = entries_by_id.remove(source_surface_id).with_context(|| {
                format!(
                    "graphics translation bundle is missing part-owned surface {source_surface_id}"
                )
            })?;
            entries.push(entry);
        }
        updated_parts.push((
            reference,
            GraphicsTranslationPart {
                schema: bundle.schema,
                part_id: existing_part.part_id,
                entries,
            },
        ));
    }
    ensure!(
        entries_by_id.is_empty(),
        "graphics translation bundle contains surfaces with no declared responsibility: {}",
        entries_by_id.keys().cloned().collect::<Vec<_>>().join(", ")
    );

    for (reference, part) in updated_parts {
        write_json_file(
            &resolve_part_path(manifest_path, &reference.relative_path)?,
            &part,
        )?;
    }

    write_json_file(
        manifest_path,
        &GraphicsTranslationManifest {
            schema: bundle.schema,
            source: bundle.source.clone(),
            scope: bundle.scope.clone(),
            population: bundle.population.clone(),
            parts: existing_manifest.parts,
        },
    )
}

pub fn write_new_graphics_translation_set(
    manifest_path: &Path,
    bundle: &GraphicsTranslationBundle,
) -> Result<()> {
    validate_manifest_filename(manifest_path)?;
    let output_root = manifest_path
        .parent()
        .context("graphics translation manifest has no parent directory")?;
    ensure!(
        !output_root.exists(),
        "refusing to overwrite graphics translation set {}",
        output_root.display()
    );
    let output_parent = output_root
        .parent()
        .context("graphics translation set has no parent directory")?;
    std::fs::create_dir_all(output_parent).with_context(|| {
        format!(
            "failed to create graphics translation output parent {}",
            output_parent.display()
        )
    })?;

    let (manifest, parts) = partition_initial_bundle(bundle)?;
    let temporary = tempfile::Builder::new()
        .prefix(".graphics-translation-")
        .tempdir_in(output_parent)
        .with_context(|| {
            format!(
                "failed to create temporary graphics translation set in {}",
                output_parent.display()
            )
        })?;
    for (reference, part) in parts {
        write_json_file(&temporary.path().join(&reference.relative_path), &part)?;
    }
    write_json_file(&temporary.path().join(MANIFEST_FILENAME), &manifest)?;
    let temporary_path = temporary.keep();
    std::fs::rename(&temporary_path, output_root).with_context(|| {
        format!(
            "failed to commit graphics translation set {}",
            output_root.display()
        )
    })?;
    Ok(())
}

fn load_parts(
    manifest_path: &Path,
    manifest: &GraphicsTranslationManifest,
) -> Result<Vec<(GraphicsTranslationPartReference, GraphicsTranslationPart)>> {
    ensure!(
        manifest.schema == 1,
        "unsupported graphics translation manifest schema"
    );
    ensure!(
        !manifest.parts.is_empty(),
        "graphics translation manifest declares no responsibility parts"
    );

    let mut part_ids = BTreeSet::new();
    let mut relative_paths = BTreeSet::new();
    let mut source_surface_ids = BTreeSet::new();
    let mut loaded = Vec::with_capacity(manifest.parts.len());
    let mut entry_count = 0usize;
    for reference in &manifest.parts {
        ensure!(
            !reference.part_id.trim().is_empty(),
            "graphics translation part has an empty identity"
        );
        ensure!(
            !reference.responsibility.trim().is_empty(),
            "graphics translation part {} has no declared responsibility",
            reference.part_id
        );
        ensure!(
            part_ids.insert(reference.part_id.clone()),
            "duplicate graphics translation part identity: {}",
            reference.part_id
        );
        ensure!(
            relative_paths.insert(reference.relative_path.clone()),
            "duplicate graphics translation part path: {}",
            reference.relative_path
        );
        let part_path = resolve_part_path(manifest_path, &reference.relative_path)?;
        let part: GraphicsTranslationPart = read_json_file(&part_path)?;
        validate_part(reference, &part, &part_path)?;
        for entry in &part.entries {
            ensure!(
                source_surface_ids.insert(entry.source.source_surface_id.clone()),
                "graphics translation surface is owned by more than one part: {}",
                entry.source.source_surface_id
            );
        }
        entry_count += part.entries.len();
        loaded.push((reference.clone(), part));
    }
    ensure!(
        entry_count == manifest.population.linked_texture_surfaces,
        "graphics translation parts contain {entry_count} surfaces, but the manifest declares {}",
        manifest.population.linked_texture_surfaces
    );
    Ok(loaded)
}

fn partition_initial_bundle(
    bundle: &GraphicsTranslationBundle,
) -> Result<(
    GraphicsTranslationManifest,
    Vec<(GraphicsTranslationPartReference, GraphicsTranslationPart)>,
)> {
    ensure!(
        bundle.schema == 1,
        "unsupported graphics translation schema"
    );
    ensure!(
        bundle.entries.len() == bundle.population.linked_texture_surfaces,
        "graphics translation bundle population differs from its entries"
    );

    let mut parts = Vec::with_capacity(bundle.entries.len());
    let mut references = Vec::with_capacity(bundle.entries.len());
    let mut part_ids = BTreeSet::new();
    for entry in &bundle.entries {
        let part_id = entry
            .source
            .source_surface_id
            .strip_prefix("linked-texture/")
            .context("graphics source surface has no linked-texture identity")?
            .replace('/', "-");
        ensure!(
            part_ids.insert(part_id.clone()),
            "duplicate graphics source surface identity while partitioning: {}",
            entry.source.source_surface_id
        );
        let reference = GraphicsTranslationPartReference {
            part_id: part_id.clone(),
            relative_path: format!("parts/pending/{part_id}.json"),
            responsibility: format!(
                "semantic responsibility pending for exact source surface {}",
                entry.source.source_surface_id
            ),
            entry_count: 1,
        };
        parts.push((
            reference.clone(),
            GraphicsTranslationPart {
                schema: bundle.schema,
                part_id,
                entries: vec![entry.clone()],
            },
        ));
        references.push(reference);
    }

    Ok((
        GraphicsTranslationManifest {
            schema: bundle.schema,
            source: bundle.source.clone(),
            scope: bundle.scope.clone(),
            population: bundle.population.clone(),
            parts: references,
        },
        parts,
    ))
}

fn validate_manifest_filename(manifest_path: &Path) -> Result<()> {
    ensure!(
        manifest_path.file_name().and_then(|name| name.to_str()) == Some(MANIFEST_FILENAME),
        "graphics translation set path must end in {MANIFEST_FILENAME}"
    );
    Ok(())
}

fn resolve_part_path(manifest_path: &Path, relative_path: &str) -> Result<PathBuf> {
    let relative_path = Path::new(relative_path);
    ensure!(
        !relative_path.as_os_str().is_empty()
            && !relative_path.is_absolute()
            && relative_path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "graphics translation part path is not a safe relative path: {}",
        relative_path.display()
    );
    let root = manifest_path
        .parent()
        .context("graphics translation manifest has no parent directory")?;
    Ok(root.join(relative_path))
}

fn validate_part(
    reference: &GraphicsTranslationPartReference,
    part: &GraphicsTranslationPart,
    part_path: &Path,
) -> Result<()> {
    ensure!(
        part.schema == 1,
        "unsupported graphics translation part schema: {}",
        part_path.display()
    );
    ensure!(
        part.part_id == reference.part_id,
        "graphics translation part identity differs from the manifest: {}",
        part_path.display()
    );
    ensure!(
        part.entries.len() == reference.entry_count,
        "graphics translation part entry count differs from the manifest: {}",
        part_path.display()
    );
    Ok(())
}

#[cfg(test)]
#[path = "translation_set_tests.rs"]
mod translation_set_tests;
