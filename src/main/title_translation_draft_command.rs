use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::title_draft::{
    TitleDraftAsset, render_title_translation_draft,
};
use puyo2_ps2_tool::graphics_translation::{
    GraphicsTranslationStatus, load_graphics_translation_set,
};
use puyo2_ps2_tool::source::{read_json_file, write_json_file};
use puyo2_ps2_tool::title_texture::render_title_textures;
use sha2::{Digest, Sha256};

use super::SourceArgs;
use super::render_source::load_texture_source;

const TITLE_SEMANTIC_ASSET_ID: &str = "title/primary-branding-sheet";

pub(super) struct TitleTranslationDraftCommand {
    pub source: SourceArgs,
    pub executable_path: String,
    pub image_path: String,
    pub translation: PathBuf,
    pub asset: PathBuf,
    pub output: PathBuf,
    pub japanese_source_mask: PathBuf,
    pub protected_nontext_mask: PathBuf,
    pub protected_english_mask: PathBuf,
    pub report: PathBuf,
}

pub(super) fn run_title_translation_draft(command: TitleTranslationDraftCommand) -> Result<()> {
    let loaded = load_texture_source(
        &command.source,
        command.executable_path,
        command.image_path.clone(),
    )?;
    let source_inputs = loaded.inputs();
    let bundle = load_graphics_translation_set(&command.translation)?;
    ensure!(
        bundle.source.source_bin_sha256 == source_inputs.source_bin_sha256
            && bundle.source.executable_iso_path == source_inputs.executable_iso_path
            && bundle.source.executable_sha256 == source_inputs.executable_sha256,
        "graphics translation bundle differs from the verified title source"
    );
    let target = bundle
        .entries
        .iter()
        .find(|entry| entry.authoring.semantic_asset_id.as_deref() == Some(TITLE_SEMANTIC_ASSET_ID))
        .context("graphics translation bundle has no semantic title target")?;
    ensure!(
        matches!(
            target.authoring.status,
            GraphicsTranslationStatus::InProgress | GraphicsTranslationStatus::NeedsHumanReview
        ),
        "title target is not in a bounded pre-review authoring state"
    );
    ensure!(
        target.source.container_iso_path == command.image_path,
        "title target uses a different image container"
    );

    let asset_bytes = std::fs::read(&command.asset)
        .with_context(|| format!("failed to hash {}", command.asset.display()))?;
    let asset_sha256 = format!("{:x}", Sha256::digest(&asset_bytes));
    ensure!(
        target.authoring.design_candidates.iter().any(|candidate| {
            candidate.path == command.asset.to_string_lossy() && candidate.sha256 == asset_sha256
        }),
        "title draft asset is not declared by the graphics translation ledger"
    );
    let asset: TitleDraftAsset = read_json_file(&command.asset)?;

    let temporary = tempfile::tempdir().context("failed to create title draft directory")?;
    let title_corpus = render_title_textures(source_inputs, temporary.path())?;
    let rendered_source = title_corpus
        .textures
        .iter()
        .find(|texture| texture.image_file_record == target.source.image_file_record)
        .context("title renderer did not reproduce the semantic title target")?;
    ensure!(
        rendered_source.width == target.source.width
            && rendered_source.height == target.source.height
            && rendered_source.linear_indices_sha256 == target.source.linear_indices_sha256,
        "rendered title source differs from the graphics translation ledger"
    );
    let rendered = render_title_translation_draft(
        PathBuf::from(&rendered_source.png_path).as_path(),
        &asset,
        &command.output,
        &command.japanese_source_mask,
        &command.protected_nontext_mask,
        &command.protected_english_mask,
    )?;
    let report = rendered.report;
    write_json_file(&command.report, &report)?;
    println!(
        "rendered pre-human-review title draft with {} changed pixels, {} changes outside the Japanese region, {} protected non-text changes, {} protected English changes, and {} colors outside the source surface to {}; wrote {}",
        report.changed_pixels,
        report.changed_pixels_outside_editable_bounds,
        report.changed_protected_nontext_pixels,
        report.changed_protected_english_pixels,
        report.output_colors_not_in_source,
        command.output.display(),
        command.report.display()
    );
    Ok(())
}
