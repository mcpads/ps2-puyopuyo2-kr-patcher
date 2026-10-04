use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::ranking_labels::{
    RankingLabelRegionReport, render_ranking_label_draft, render_ranking_label_region_masks,
};
use puyo2_ps2_tool::graphics_translation::{
    GraphicsTranslationStatus, load_graphics_translation_set,
};
use puyo2_ps2_tool::rank_texture::render_rank_textures;
use puyo2_ps2_tool::source::write_json_file;
use sha2::{Digest, Sha256};

use super::SourceArgs;
use super::render_source::load_texture_source;

const RANKING_LABEL_ASSET_ID: &str = "ranking/interface-label-sheet";
const RANKING_LABEL_IMAGE_RECORD: usize = 120;

pub(super) struct RankingLabelRegionCommand {
    pub source: SourceArgs,
    pub executable_path: String,
    pub image_path: String,
    pub translation: PathBuf,
    pub solo_label_mask: PathBuf,
    pub endless_label_mask: PathBuf,
    pub protected_mask: PathBuf,
    pub report: PathBuf,
}

pub(super) struct RankingLabelDraftCommand {
    pub source: SourceArgs,
    pub executable_path: String,
    pub image_path: String,
    pub translation: PathBuf,
    pub font_config: PathBuf,
    pub font: PathBuf,
    pub output: PathBuf,
    pub report: PathBuf,
}

pub(super) fn run_ranking_label_region_render(command: RankingLabelRegionCommand) -> Result<()> {
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
        "graphics translation bundle differs from the verified ranking source"
    );
    let target = bundle
        .entries
        .iter()
        .find(|entry| entry.authoring.semantic_asset_id.as_deref() == Some(RANKING_LABEL_ASSET_ID))
        .context("graphics translation bundle has no ranking label target")?;
    ensure!(
        matches!(
            target.authoring.status,
            GraphicsTranslationStatus::NeedsRegionInventory
                | GraphicsTranslationStatus::InProgress
                | GraphicsTranslationStatus::NeedsHumanReview
        ) && target.source.container_iso_path == command.image_path
            && target.source.image_file_record == RANKING_LABEL_IMAGE_RECORD,
        "ranking label target is not the expected pre-review source surface"
    );

    let temporary = tempfile::tempdir().context("failed to create ranking label directory")?;
    let corpus = render_rank_textures(source_inputs, temporary.path())?;
    let source = corpus
        .textures
        .iter()
        .find(|texture| texture.image_file_record == RANKING_LABEL_IMAGE_RECORD)
        .context("ranking renderer did not reproduce the label source surface")?;
    ensure!(
        source.width == target.source.width
            && source.height == target.source.height
            && source.linear_indices_sha256 == target.source.linear_indices_sha256,
        "rendered ranking label source differs from the graphics translation ledger"
    );

    let report = render_ranking_label_region_masks(
        Path::new(&source.png_path),
        &command.solo_label_mask,
        &command.endless_label_mask,
        &command.protected_mask,
    )?;
    write_json_file(&command.report, &report)?;
    println!(
        "rendered {} ranking Japanese label masks and one protected complement; wrote {}",
        report.regions.len(),
        command.report.display()
    );
    Ok(())
}

pub(super) fn run_ranking_label_draft(command: RankingLabelDraftCommand) -> Result<()> {
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
        "graphics translation bundle differs from the verified ranking source"
    );
    let target = bundle
        .entries
        .iter()
        .find(|entry| entry.authoring.semantic_asset_id.as_deref() == Some(RANKING_LABEL_ASSET_ID))
        .context("graphics translation bundle has no ranking label target")?;
    ensure!(
        matches!(
            target.authoring.status,
            GraphicsTranslationStatus::InProgress | GraphicsTranslationStatus::NeedsHumanReview
        ) && target.source.container_iso_path == command.image_path
            && target.source.image_file_record == RANKING_LABEL_IMAGE_RECORD,
        "ranking label target is not the expected pre-review source surface"
    );
    ensure!(
        target.authoring.human_approval.is_none() && target.authoring.independent_review.is_none(),
        "ranking label draft command only admits the pre-human-review authoring state"
    );

    let temporary = tempfile::tempdir().context("failed to create ranking label directory")?;
    let corpus = render_rank_textures(source_inputs, temporary.path())?;
    let source = corpus
        .textures
        .iter()
        .find(|texture| texture.image_file_record == RANKING_LABEL_IMAGE_RECORD)
        .context("ranking renderer did not reproduce the label source surface")?;
    ensure!(
        source.width == target.source.width
            && source.height == target.source.height
            && source.linear_indices_sha256 == target.source.linear_indices_sha256,
        "rendered ranking label source differs from the graphics translation ledger"
    );
    let region_report = render_ranking_label_region_masks(
        Path::new(&source.png_path),
        &temporary.path().join("solo-mode-label-source.mask.png"),
        &temporary.path().join("endless-mode-label-source.mask.png"),
        &temporary.path().join("protected-outside-japanese.mask.png"),
    )?;
    verify_ledger_regions(target, &region_report)?;

    let report = render_ranking_label_draft(
        Path::new(&source.png_path),
        &command.font_config,
        &command.font,
        &command.output,
    )?;
    if target.authoring.status == GraphicsTranslationStatus::NeedsHumanReview {
        let replacement = target
            .authoring
            .replacement_image
            .as_ref()
            .context("review-ready ranking target has no declared replacement")?;
        let replacement_bytes = std::fs::read(&replacement.path)
            .with_context(|| format!("failed to read ranking replacement {}", replacement.path))?;
        ensure!(
            replacement.sha256 == format!("{:x}", Sha256::digest(&replacement_bytes))
                && replacement.sha256 == report.output_png_sha256,
            "declared ranking replacement differs from the deterministic draft"
        );
    }
    write_json_file(&command.report, &report)?;
    println!(
        "rendered deterministic pre-human-review ranking draft with {} changed pixels and {} protected changes to {}; wrote {}",
        report.changed_pixels,
        report.changed_pixels_outside_label_windows,
        command.output.display(),
        command.report.display()
    );
    Ok(())
}

fn verify_ledger_regions(
    target: &puyo2_ps2_tool::graphics_translation::GraphicsTranslationEntry,
    report: &RankingLabelRegionReport,
) -> Result<()> {
    ensure!(
        target.authoring.region_inventory_complete
            && target.authoring.regions.len() == report.regions.len(),
        "ranking label region inventory is incomplete"
    );
    for region in &report.regions {
        let declared = target
            .authoring
            .regions
            .iter()
            .find(|declared| declared.region_id == region.region_id)
            .with_context(|| format!("ranking region is not declared: {}", region.region_id))?;
        ensure!(
            declared.source_text == region.source_text
                && declared.korean_text.as_deref() == Some(region.korean_text)
                && declared.bounds == region.bounds
                && declared.source_pixel_mask.sha256 == region.source_pixel_mask.sha256,
            "ranking region differs from its exact-source inventory: {}",
            region.region_id
        );
    }
    let protected = target
        .authoring
        .protected_english_mask
        .as_ref()
        .context("ranking label target has no protected complement")?;
    ensure!(
        protected.sha256 == report.protected_outside_japanese_mask.sha256,
        "ranking protected complement differs from the exact-source inventory"
    );
    Ok(())
}
