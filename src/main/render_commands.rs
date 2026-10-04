use std::path::PathBuf;

use anyhow::{Context, Result};
use puyo2_ps2_tool::continue_texture::render_continue_textures;
use puyo2_ps2_tool::font_atlas::nazo::render_nazo_font_atlas;
use puyo2_ps2_tool::font_atlas::render_font_atlas;
use puyo2_ps2_tool::font_atlas::tutorial::render_tutorial_font_atlas;
use puyo2_ps2_tool::omake_texture::render_omake_textures;
use puyo2_ps2_tool::rank_texture::render_rank_textures;
use puyo2_ps2_tool::rule_texture::render_rule_textures;
use puyo2_ps2_tool::source::{find_inventory_file, read_inventory_file, write_json_file};
use puyo2_ps2_tool::texture_preview_corpus::{
    TexturePreviewContainerInput, TexturePreviewCorpusInputs, render_texture_preview_corpus,
};
use puyo2_ps2_tool::title_texture::render_title_textures;
use puyo2_ps2_tool::unlinked_texture_corpus::{
    LinkedTextureContainerInput, UnlinkedTextureCorpusInputs, render_unlinked_texture_corpus,
};

use super::render_source::load_texture_source;
use super::{SourceArgs, inspect};

pub(super) fn run_title_texture_render(
    source: SourceArgs,
    executable_path: String,
    image_path: String,
    output_directory: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let loaded = load_texture_source(&source, executable_path, image_path)?;
    let rendered = render_title_textures(loaded.inputs(), &output_directory)?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} title textures to {} and wrote {}",
        rendered.textures.len(),
        output_directory.display(),
        report.display()
    );
    Ok(())
}

pub(super) fn run_font_atlas_render(
    source: SourceArgs,
    executable_path: String,
    image_path: String,
    output: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let loaded = load_texture_source(&source, executable_path, image_path)?;
    let rendered = render_font_atlas(loaded.inputs(), &output)?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} fullwidth and {} halfwidth glyph mappings to {} and wrote {}",
        rendered.fullwidth.glyphs.len(),
        rendered.halfwidth.glyphs.len(),
        output.display(),
        report.display()
    );
    Ok(())
}

pub(super) fn run_nazo_font_atlas_render(
    source: SourceArgs,
    executable_path: String,
    image_path: String,
    output: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let loaded = load_texture_source(&source, executable_path, image_path)?;
    let rendered = render_nazo_font_atlas(loaded.inputs(), &output)?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} Nazo fullwidth and {} halfwidth glyph mappings to {} and wrote {}",
        rendered.fullwidth.glyphs.len(),
        rendered.halfwidth.glyphs.len(),
        output.display(),
        report.display()
    );
    Ok(())
}

pub(super) fn run_tutorial_font_atlas_render(
    source: SourceArgs,
    executable_path: String,
    image_path: String,
    output: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let loaded = load_texture_source(&source, executable_path, image_path)?;
    let rendered = render_tutorial_font_atlas(loaded.inputs(), &output)?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} tutorial fullwidth glyph mappings to {} and wrote {}",
        rendered.fullwidth.glyphs.len(),
        output.display(),
        report.display()
    );
    Ok(())
}

pub(super) fn run_rank_texture_render(
    source: SourceArgs,
    executable_path: String,
    image_path: String,
    output_directory: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let loaded = load_texture_source(&source, executable_path, image_path)?;
    let rendered = render_rank_textures(loaded.inputs(), &output_directory)?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} ranking texture pairs to {} and wrote {}",
        rendered.textures.len(),
        output_directory.display(),
        report.display()
    );
    Ok(())
}

pub(super) fn run_continue_texture_render(
    source: SourceArgs,
    executable_path: String,
    image_path: String,
    output_directory: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let loaded = load_texture_source(&source, executable_path, image_path)?;
    let rendered = render_continue_textures(loaded.inputs(), &output_directory)?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} continue-screen textures to {} and wrote {}",
        rendered.textures.len(),
        output_directory.display(),
        report.display()
    );
    Ok(())
}

pub(super) fn run_omake_texture_render(
    source: SourceArgs,
    executable_path: String,
    image_path: String,
    output_directory: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let loaded = load_texture_source(&source, executable_path, image_path)?;
    let rendered = render_omake_textures(loaded.inputs(), &output_directory)?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} Omake-menu textures to {} and wrote {}",
        rendered.textures.len(),
        output_directory.display(),
        report.display()
    );
    Ok(())
}

pub(super) fn run_rule_texture_render(
    source: SourceArgs,
    executable_path: String,
    image_path: String,
    output_directory: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let loaded = load_texture_source(&source, executable_path, image_path)?;
    let rendered = render_rule_textures(loaded.inputs(), &output_directory)?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} rule-selection textures to {} and wrote {}",
        rendered.textures.len(),
        output_directory.display(),
        report.display()
    );
    Ok(())
}

pub(super) fn run_texture_preview_corpus(
    source: SourceArgs,
    executable_iso_path: String,
    image_paths: [String; 3],
    output_directory: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let mut loaded_containers = Vec::with_capacity(image_paths.len());
    for (file_id, iso_path) in image_paths.into_iter().enumerate() {
        let bytes = read_inventory_file(&source.bin, &inventory, &iso_path)?;
        let sha256 = find_inventory_file(&inventory, &iso_path)?
            .sha256
            .clone()
            .with_context(|| format!("image inventory entry has no SHA-256: {iso_path}"))?;
        loaded_containers.push((file_id, iso_path, sha256, bytes));
    }
    let container_inputs = loaded_containers
        .iter()
        .map(
            |(file_id, iso_path, sha256, bytes)| TexturePreviewContainerInput {
                file_id: *file_id,
                iso_path: iso_path.clone(),
                sha256: sha256.clone(),
                bytes,
            },
        )
        .collect();
    let rendered = render_texture_preview_corpus(
        TexturePreviewCorpusInputs {
            source_bin_sha256: inventory.source.bin.sha256,
            executable_iso_path,
            executable_sha256,
            executable: &executable,
            containers: container_inputs,
        },
        &output_directory,
    )?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} preview variants for all {} source images from {} load/registration pair instances and {} residual variants; resolved {} consumer-paired tables; skipped {} tables; wrote {}",
        rendered.summary.unique_previews,
        rendered.summary.source_images,
        rendered.summary.pair_instances,
        rendered.summary.residual_preview_variants,
        rendered.summary.consumer_pair_tables,
        rendered.summary.skipped_mismatched_tables,
        report.display()
    );
    Ok(())
}

pub(super) fn run_unlinked_texture_corpus(
    source: SourceArgs,
    executable_iso_path: String,
    container_iso_path: String,
    linked_paths: [String; 3],
    output_directory: PathBuf,
    report: PathBuf,
) -> Result<()> {
    let inventory = inspect(&source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let container = read_inventory_file(&source.bin, &inventory, &container_iso_path)?;
    let container_sha256 = find_inventory_file(&inventory, &container_iso_path)?
        .sha256
        .clone()
        .with_context(|| format!("texture inventory entry has no SHA-256: {container_iso_path}"))?;
    let file_list_symbols = ["FileList_Image", "FileList_Image2", "FileList_Image3"];
    let mut loaded_linked = Vec::with_capacity(linked_paths.len());
    for (iso_path, file_list_symbol) in linked_paths.into_iter().zip(file_list_symbols) {
        let bytes = read_inventory_file(&source.bin, &inventory, &iso_path)?;
        let sha256 = find_inventory_file(&inventory, &iso_path)?
            .sha256
            .clone()
            .with_context(|| format!("texture inventory entry has no SHA-256: {iso_path}"))?;
        loaded_linked.push((iso_path, sha256, bytes, file_list_symbol.to_string()));
    }
    let linked_containers = loaded_linked
        .iter()
        .map(
            |(iso_path, sha256, bytes, file_list_symbol)| LinkedTextureContainerInput {
                iso_path: iso_path.clone(),
                sha256: sha256.clone(),
                bytes,
                file_list_symbol: file_list_symbol.clone(),
            },
        )
        .collect();
    let rendered = render_unlinked_texture_corpus(
        UnlinkedTextureCorpusInputs {
            source_bin_sha256: inventory.source.bin.sha256,
            executable_iso_path,
            executable_sha256,
            executable: &executable,
            container_iso_path,
            container_sha256,
            container: &container,
            linked_containers,
        },
        &output_directory,
    )?;
    write_json_file(&report, &rendered)?;
    println!(
        "rendered {} dimension candidates for {} unlinked images; {} exact linked pairs and {} linked palettes; wrote {}",
        rendered.summary.preview_candidates,
        rendered.summary.images,
        rendered.summary.images_with_exact_linked_pair_match,
        rendered.summary.images_with_linked_palette_match,
        report.display()
    );
    Ok(())
}
