use std::path::{Path, PathBuf};

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
use puyo2_ps2_tool::memory_card_metadata::{MemoryCardMetadataInputs, survey_memory_card_metadata};
use puyo2_ps2_tool::movie_corpus::{
    MovieCorpus, MovieCorpusInputs, MovieInput, MovieLanguageCatalog, survey_movie_corpus,
};
use puyo2_ps2_tool::source::{
    find_inventory_file, read_inventory_file, read_json_file, write_json_file,
};
use puyo2_ps2_tool::texture_language_survey::{
    TextureLanguageCatalog, classify_texture_previews, verify_texture_preview_files,
};
use puyo2_ps2_tool::texture_load_calls::{TextureLoadCallInputs, survey_texture_load_calls};
use puyo2_ps2_tool::texture_load_census::{TextureLoadCensusInputs, survey_texture_load_tables};
use puyo2_ps2_tool::texture_preview_corpus::TexturePreviewCorpus;
use puyo2_ps2_tool::unlinked_texture_corpus::UnlinkedTextureCorpus;
use puyo2_ps2_tool::unlinked_texture_corpus::language_survey::{
    UnlinkedTextureLanguageCatalog, classify_unlinked_texture_previews,
    verify_unlinked_texture_previews,
};

use super::{SourceArgs, inspect};

pub(super) fn run_executable_font_consumer_survey(
    source: SourceArgs,
    executable_iso_path: String,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let report = survey_executable_font_consumers(ExecutableFontConsumerInputs {
        source_bin_sha256: inventory.source.bin.sha256,
        executable_iso_path,
        executable_sha256,
        executable: &executable,
    })?;
    write_json_file(&output, &report)?;
    println!(
        "surveyed {} typed lookup calls across {} font families with {} unresolved consumer paths; wrote {}",
        report.summary.typed_lookup_calls,
        report.summary.font_families,
        report.summary.unresolved_font_consumer_paths,
        output.display()
    );
    Ok(())
}

pub(super) fn run_executable_computed_reference_survey(
    source: SourceArgs,
    executable_iso_path: String,
    catalog_path: PathBuf,
    code_data_review_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let string_catalog: ExecutableStringReviewCatalog = read_json_file(&catalog_path)?;
    let code_data_catalog: ExecutableCodeDataReviewCatalog =
        read_json_file(&code_data_review_path)?;
    let report = survey_executable_computed_references(
        ExecutableComputedReferenceInputs {
            source_bin_sha256: inventory.source.bin.sha256,
            executable_iso_path,
            executable_sha256,
            executable: &executable,
        },
        &string_catalog,
        &code_data_catalog,
    )?;
    write_json_file(&output, &report)?;
    println!(
        "typed {} executable words across {} basic blocks; found {} computed reference edges ({} require control-flow propagation) and {} newly referenced candidates; wrote {}",
        report.summary.unique_typed_instruction_words,
        report.summary.straight_line_basic_blocks,
        report.summary.candidate_reference_edges,
        report.summary.references_requiring_control_flow_propagation,
        report.summary.newly_referenced_candidates,
        output.display()
    );
    Ok(())
}

pub(super) fn run_executable_gp_reference_survey(
    source: SourceArgs,
    executable_iso_path: String,
    catalog_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let catalog: ExecutableStringReviewCatalog = read_json_file(&catalog_path)?;
    let report = survey_executable_gp_references(
        ExecutableGpReferenceInputs {
            source_bin_sha256: inventory.source.bin.sha256,
            executable_iso_path,
            executable_sha256,
            executable: &executable,
        },
        &catalog,
    )?;
    write_json_file(&output, &report)?;
    println!(
        "typed {} GP-relative instructions across {} unique text words; found {} direct and {} pointer-load references; wrote {}",
        report.summary.typed_gp_relative_instructions,
        report.summary.unique_text_words,
        report.summary.direct_gp_address_references,
        report.summary.gp_pointer_load_references,
        output.display()
    );
    Ok(())
}

pub(super) fn run_executable_storage_gap_survey(
    source: SourceArgs,
    executable_iso_path: String,
    catalog_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let catalog: ExecutableStorageGapReviewCatalog = read_json_file(&catalog_path)?;
    let report = survey_executable_storage_gaps(
        ExecutableStorageGapInputs {
            source_bin_sha256: inventory.source.bin.sha256,
            executable_iso_path,
            executable_sha256,
            executable: &executable,
        },
        &catalog,
    )?;
    write_json_file(&output, &report)?;
    println!(
        "classified {} Japanese-looking storage-gap candidates as {} display text and {} binary; wrote {}",
        report.summary.storage_gap_candidates,
        report.summary.displayed_japanese_text,
        report.summary.non_string_binary,
        output.display()
    );
    Ok(())
}

pub(super) fn run_memory_card_metadata_survey(
    source: SourceArgs,
    executable_iso_path: String,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let report = survey_memory_card_metadata(MemoryCardMetadataInputs {
        source_bin_sha256: inventory.source.bin.sha256,
        executable_iso_path,
        executable_sha256,
        executable: &executable,
    })?;
    write_json_file(&output, &report)?;
    println!(
        "traced {} Japanese memory-card title through {} to {}; wrote {}",
        report.population.japanese_title_entries,
        report.creation_queue.write_buffer_symbol,
        report.dispatcher.write_api_symbol,
        output.display()
    );
    Ok(())
}

pub(super) fn run_executable_string_census(
    source: SourceArgs,
    executable_iso_path: String,
    catalog_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let catalog: ExecutableStringReviewCatalog = read_json_file(&catalog_path)?;
    let report = census_executable_strings(
        ExecutableStringCensusInputs {
            source_bin_sha256: inventory.source.bin.sha256,
            executable_iso_path,
            executable_sha256,
            executable: &executable,
        },
        &catalog,
    )?;
    write_json_file(&output, &report)?;
    println!(
        "classified {} Japanese-script symbol candidates; {} have {} typed address and {} pointer references; wrote {}",
        report.summary.japanese_script_candidates,
        report.summary.referenced_candidates,
        report.summary.typed_address_references,
        report.summary.pointer_references,
        output.display()
    );
    Ok(())
}

pub(super) fn run_texture_load_census(
    source: SourceArgs,
    executable_iso_path: String,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let report = survey_texture_load_tables(TextureLoadCensusInputs {
        source_bin_sha256: inventory.source.bin.sha256,
        executable_iso_path,
        executable_sha256,
        executable: &executable,
    })?;
    write_json_file(&output, &report)?;
    println!(
        "classified {} texture-table candidates into {} load tables and {} other named tables; wrote {}",
        report.summary.table_candidates,
        report.summary.load_tables,
        report.summary.other_named_tables,
        output.display()
    );
    Ok(())
}

pub(super) fn run_texture_load_call_survey(
    source: SourceArgs,
    executable_iso_path: String,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let report = survey_texture_load_calls(TextureLoadCallInputs {
        source_bin_sha256: inventory.source.bin.sha256,
        executable_iso_path,
        executable_sha256,
        executable: &executable,
    })?;
    write_json_file(&output, &report)?;
    println!(
        "resolved {} texture read calls through {} direct and {} pointer-table arguments; {} load tables are reachable; wrote {}",
        report.summary.texture_load_calls,
        report.summary.direct_load_table_calls,
        report.summary.pointer_table_calls,
        report.summary.reachable_load_tables,
        output.display()
    );
    Ok(())
}

pub(super) fn run_texture_language_survey(
    source: SourceArgs,
    preview_report: PathBuf,
    catalog_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let previews: TexturePreviewCorpus = read_json_file(&preview_report)?;
    ensure!(
        previews.source_bin_sha256 == inventory.source.bin.sha256,
        "texture preview source BIN hash differs from verified source"
    );
    let executable = find_inventory_file(&inventory, &previews.executable_iso_path)?;
    ensure!(
        executable.sha256.as_deref() == Some(previews.executable_sha256.as_str()),
        "texture preview executable hash differs from verified inventory"
    );
    for container in &previews.containers {
        let inventory_file = find_inventory_file(&inventory, &container.iso_path)?;
        ensure!(
            inventory_file.sha256.as_deref() == Some(container.sha256.as_str()),
            "texture preview container hash differs from verified inventory: {}",
            container.iso_path
        );
    }
    verify_texture_preview_files(&previews)?;
    let catalog: TextureLanguageCatalog = read_json_file(&catalog_path)?;
    let report = classify_texture_previews(&previews, &catalog)?;
    write_json_file(&output, &report)?;
    println!(
        "classified {} preview images: {} Japanese candidates, {} admitted targets, {} English-only, {} non-text, {} unresolved; wrote {}",
        report.summary.unique_images,
        report.summary.japanese_candidate_images,
        report.summary.admitted_translation_targets,
        report.summary.english_only_images,
        report.summary.non_text_images,
        report.summary.unresolved_images,
        output.display()
    );
    Ok(())
}

pub(super) fn run_unlinked_texture_language_survey(
    source: SourceArgs,
    preview_report: PathBuf,
    catalog_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let previews: UnlinkedTextureCorpus = read_json_file(&preview_report)?;
    ensure!(
        previews.source_bin_sha256 == inventory.source.bin.sha256,
        "unlinked texture preview source BIN differs from verified source"
    );
    let executable = find_inventory_file(&inventory, &previews.executable_iso_path)?;
    ensure!(
        executable.sha256.as_deref() == Some(previews.executable_sha256.as_str()),
        "unlinked texture executable differs from verified inventory"
    );
    let container = find_inventory_file(&inventory, &previews.container_iso_path)?;
    ensure!(
        container.sha256.as_deref() == Some(previews.container_sha256.as_str()),
        "unlinked texture container differs from verified inventory"
    );
    verify_unlinked_texture_previews(&previews)?;
    let catalog: UnlinkedTextureLanguageCatalog = read_json_file(&catalog_path)?;
    let report = classify_unlinked_texture_previews(&previews, &catalog)?;
    ensure!(
        report.summary.unresolved_images == 0,
        "unlinked texture language survey still has unresolved images"
    );
    write_json_file(&output, &report)?;
    println!(
        "classified {} unlinked images into {} Japanese candidates, {} English-only images, and {} non-text images; {} dimensions remain unresolved; wrote {}",
        report.summary.images,
        report.summary.japanese_candidate_images,
        report.summary.english_only_images,
        report.summary.non_text_images,
        report.summary.unresolved_dimension_candidates,
        output.display()
    );
    Ok(())
}

pub(super) fn run_movie_corpus_survey(
    source: SourceArgs,
    executable_iso_path: String,
    catalog_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let report = load_movie_corpus(&source, &executable_iso_path, &catalog_path)?;
    write_json_file(&output, &report)?;
    println!(
        "surveyed {} PSS movies and {} MPEG-2 frames; {} product targets, {} excluded other-title promotions; wrote {}",
        report.summary.movies,
        report.summary.video_frames,
        report.summary.japanese_target_movies,
        report.summary.excluded_other_title_promotion_movies,
        output.display()
    );
    Ok(())
}

pub(super) fn load_movie_corpus(
    source: &SourceArgs,
    executable_iso_path: &str,
    catalog_path: &Path,
) -> Result<MovieCorpus> {
    let inventory = inspect(source)?;
    let executable = read_inventory_file(&source.bin, &inventory, executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let mut movie_paths = inventory
        .entries
        .iter()
        .filter(|entry| entry.path.starts_with("/MOVIE/") && entry.path.ends_with(".PSS;1"))
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    movie_paths.sort();
    ensure!(!movie_paths.is_empty(), "ISO inventory has no PSS movies");
    let mut loaded_movies = Vec::with_capacity(movie_paths.len());
    for iso_path in movie_paths {
        let bytes = read_inventory_file(&source.bin, &inventory, &iso_path)?;
        let sha256 = find_inventory_file(&inventory, &iso_path)?
            .sha256
            .clone()
            .with_context(|| format!("movie inventory entry has no SHA-256: {iso_path}"))?;
        loaded_movies.push((iso_path, sha256, bytes));
    }
    let movies = loaded_movies
        .iter()
        .map(|(iso_path, sha256, bytes)| MovieInput {
            iso_path: iso_path.clone(),
            sha256: sha256.clone(),
            bytes,
        })
        .collect();
    let catalog: MovieLanguageCatalog = read_json_file(catalog_path)?;
    survey_movie_corpus(
        MovieCorpusInputs {
            source_bin_sha256: inventory.source.bin.sha256,
            executable_iso_path: executable_iso_path.to_string(),
            executable_sha256,
            executable: &executable,
            movies,
        },
        &catalog,
    )
}
