use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::executable_computed_references::{
    ExecutableCodeDataReviewCatalog, ExecutableComputedReferenceInputs,
    survey_executable_computed_references,
};
use puyo2_ps2_tool::executable_font_consumers::{
    ExecutableFontConsumerInputs, survey_executable_font_consumers,
};
use puyo2_ps2_tool::executable_gp_references::{
    ExecutableGpReferenceInputs, survey_executable_gp_references,
};
use puyo2_ps2_tool::executable_storage_gaps::{
    ExecutableStorageGapInputs, ExecutableStorageGapReviewCatalog, survey_executable_storage_gaps,
};
use puyo2_ps2_tool::executable_string_census::{
    ExecutableStringCensusInputs, ExecutableStringReviewCatalog, census_executable_strings,
};
use puyo2_ps2_tool::movie_corpus::{
    MovieCorpusInputs, MovieInput, MovieLanguageCatalog, survey_movie_corpus,
};
use puyo2_ps2_tool::source::{
    find_inventory_file, read_inventory_file, read_json_file, write_json_file,
};
use puyo2_ps2_tool::texture_language_survey::{
    TextureLanguageCatalog, classify_texture_previews, verify_texture_preview_files,
};
use puyo2_ps2_tool::texture_load_calls::{TextureLoadCallInputs, survey_texture_load_calls};
use puyo2_ps2_tool::texture_preview_corpus::TexturePreviewCorpus;
use puyo2_ps2_tool::translation_targets::{TranslationTargetInputs, compose_translation_targets};
use puyo2_ps2_tool::unlinked_texture_corpus::UnlinkedTextureCorpus;
use puyo2_ps2_tool::unlinked_texture_corpus::language_survey::{
    UnlinkedTextureLanguageCatalog, classify_unlinked_texture_previews,
    verify_unlinked_texture_previews,
};

use super::{SourceArgs, inspect};

pub(super) struct TranslationTargetCommand {
    pub source: SourceArgs,
    pub executable_path: String,
    pub executable_string_catalog: PathBuf,
    pub storage_gap_catalog: PathBuf,
    pub code_data_review: PathBuf,
    pub texture_preview_report: PathBuf,
    pub texture_catalog: PathBuf,
    pub unlinked_texture_preview_report: PathBuf,
    pub unlinked_texture_catalog: PathBuf,
    pub movie_catalog: PathBuf,
    pub output: PathBuf,
}

pub(super) fn run_translation_target_survey(command: TranslationTargetCommand) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let executable =
        read_inventory_file(&command.source.bin, &inventory, &command.executable_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &command.executable_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!(
                "executable inventory entry has no SHA-256: {}",
                command.executable_path
            )
        })?;
    let source_bin_sha256 = inventory.source.bin.sha256.clone();

    let string_catalog: ExecutableStringReviewCatalog =
        read_json_file(&command.executable_string_catalog)?;
    let storage_gap_catalog: ExecutableStorageGapReviewCatalog =
        read_json_file(&command.storage_gap_catalog)?;
    let code_data_catalog: ExecutableCodeDataReviewCatalog =
        read_json_file(&command.code_data_review)?;
    let executable_strings = census_executable_strings(
        ExecutableStringCensusInputs {
            source_bin_sha256: source_bin_sha256.clone(),
            executable_iso_path: command.executable_path.clone(),
            executable_sha256: executable_sha256.clone(),
            executable: &executable,
        },
        &string_catalog,
    )?;
    let storage_gaps = survey_executable_storage_gaps(
        ExecutableStorageGapInputs {
            source_bin_sha256: source_bin_sha256.clone(),
            executable_iso_path: command.executable_path.clone(),
            executable_sha256: executable_sha256.clone(),
            executable: &executable,
        },
        &storage_gap_catalog,
    )?;
    let gp_references = survey_executable_gp_references(
        ExecutableGpReferenceInputs {
            source_bin_sha256: source_bin_sha256.clone(),
            executable_iso_path: command.executable_path.clone(),
            executable_sha256: executable_sha256.clone(),
            executable: &executable,
        },
        &string_catalog,
    )?;
    let computed_references = survey_executable_computed_references(
        ExecutableComputedReferenceInputs {
            source_bin_sha256: source_bin_sha256.clone(),
            executable_iso_path: command.executable_path.clone(),
            executable_sha256: executable_sha256.clone(),
            executable: &executable,
        },
        &string_catalog,
        &code_data_catalog,
    )?;
    let font_consumers = survey_executable_font_consumers(ExecutableFontConsumerInputs {
        source_bin_sha256: source_bin_sha256.clone(),
        executable_iso_path: command.executable_path.clone(),
        executable_sha256: executable_sha256.clone(),
        executable: &executable,
    })?;
    let texture_load_calls = survey_texture_load_calls(TextureLoadCallInputs {
        source_bin_sha256: source_bin_sha256.clone(),
        executable_iso_path: command.executable_path.clone(),
        executable_sha256: executable_sha256.clone(),
        executable: &executable,
    })?;

    let texture_preview_corpus: TexturePreviewCorpus =
        read_json_file(&command.texture_preview_report)?;
    verify_preview_container_identities(&inventory, &texture_preview_corpus)?;
    verify_texture_preview_files(&texture_preview_corpus)?;
    let texture_catalog: TextureLanguageCatalog = read_json_file(&command.texture_catalog)?;
    let linked_textures = classify_texture_previews(&texture_preview_corpus, &texture_catalog)?;

    let unlinked_texture_corpus: UnlinkedTextureCorpus =
        read_json_file(&command.unlinked_texture_preview_report)?;
    verify_unlinked_container_identity(&inventory, &unlinked_texture_corpus)?;
    verify_unlinked_texture_previews(&unlinked_texture_corpus)?;
    let unlinked_texture_catalog: UnlinkedTextureLanguageCatalog =
        read_json_file(&command.unlinked_texture_catalog)?;
    let unlinked_textures =
        classify_unlinked_texture_previews(&unlinked_texture_corpus, &unlinked_texture_catalog)?;

    let movie_catalog: MovieLanguageCatalog = read_json_file(&command.movie_catalog)?;
    let loaded_movies = load_movies(&command, &inventory)?;
    let movies = loaded_movies
        .iter()
        .map(|(iso_path, sha256, bytes)| MovieInput {
            iso_path: iso_path.clone(),
            sha256: sha256.clone(),
            bytes,
        })
        .collect();
    let movies = survey_movie_corpus(
        MovieCorpusInputs {
            source_bin_sha256: source_bin_sha256.clone(),
            executable_iso_path: command.executable_path.clone(),
            executable_sha256: executable_sha256.clone(),
            executable: &executable,
            movies,
        },
        &movie_catalog,
    )?;

    let report = compose_translation_targets(TranslationTargetInputs {
        source_bin_sha256: &source_bin_sha256,
        executable_iso_path: &command.executable_path,
        executable_sha256: &executable_sha256,
        executable_strings: &executable_strings,
        storage_gaps: &storage_gaps,
        gp_references: &gp_references,
        computed_references: &computed_references,
        font_consumers: &font_consumers,
        texture_load_calls: &texture_load_calls,
        linked_textures: &linked_textures,
        unlinked_textures: &unlinked_textures,
        movies: &movies,
    })?;
    write_json_file(&command.output, &report)?;
    println!(
        "admitted {} exact-source target assets ({} executable strings, {} linked textures, {} movies), excluded {} Japanese candidates with explicit reasons, and left {} unresolved decisions; wrote {}",
        report.summary.admitted_target_assets,
        report.summary.executable_text_targets,
        report.summary.linked_texture_targets,
        report.summary.movie_targets,
        report.summary.excluded_japanese_candidates,
        report.summary.unresolved_target_decisions,
        command.output.display()
    );
    Ok(())
}

fn verify_preview_container_identities(
    inventory: &puyo2_ps2_tool::source::DiscInventory,
    corpus: &TexturePreviewCorpus,
) -> Result<()> {
    ensure!(
        corpus.source_bin_sha256 == inventory.source.bin.sha256,
        "texture preview corpus differs from the verified source BIN"
    );
    for container in &corpus.containers {
        let inventory_sha256 = find_inventory_file(inventory, &container.iso_path)?
            .sha256
            .as_deref()
            .with_context(|| {
                format!(
                    "texture inventory entry has no SHA-256: {}",
                    container.iso_path
                )
            })?;
        ensure!(
            container.sha256 == inventory_sha256,
            "texture preview container differs from the verified inventory: {}",
            container.iso_path
        );
    }
    Ok(())
}

fn verify_unlinked_container_identity(
    inventory: &puyo2_ps2_tool::source::DiscInventory,
    corpus: &UnlinkedTextureCorpus,
) -> Result<()> {
    ensure!(
        corpus.source_bin_sha256 == inventory.source.bin.sha256,
        "unlinked texture corpus differs from the verified source BIN"
    );
    let inventory_sha256 = find_inventory_file(inventory, &corpus.container_iso_path)?
        .sha256
        .as_deref()
        .context("unlinked texture inventory entry has no SHA-256")?;
    ensure!(
        corpus.container_sha256 == inventory_sha256,
        "unlinked texture corpus differs from the verified inventory"
    );
    Ok(())
}

fn load_movies(
    command: &TranslationTargetCommand,
    inventory: &puyo2_ps2_tool::source::DiscInventory,
) -> Result<Vec<(String, String, Vec<u8>)>> {
    let mut paths = inventory
        .entries
        .iter()
        .filter(|entry| entry.path.starts_with("/MOVIE/") && entry.path.ends_with(".PSS;1"))
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    ensure!(!paths.is_empty(), "ISO inventory has no PSS movies");
    paths
        .into_iter()
        .map(|iso_path| {
            let bytes = read_inventory_file(&command.source.bin, inventory, &iso_path)?;
            let sha256 = find_inventory_file(inventory, &iso_path)?
                .sha256
                .clone()
                .with_context(|| format!("movie inventory entry has no SHA-256: {iso_path}"))?;
            Ok((iso_path, sha256, bytes))
        })
        .collect()
}
