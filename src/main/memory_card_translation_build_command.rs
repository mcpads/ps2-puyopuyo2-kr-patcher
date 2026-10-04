use std::path::PathBuf;

use anyhow::Result;
use puyo2_ps2_tool::memory_card_translation_build::{
    MemoryCardTranslationBuildInputs, build_memory_card_translation_development,
};
use puyo2_ps2_tool::source::{read_inventory_file, write_json_file};

use super::{SourceArgs, inspect};

pub(super) struct MemoryCardTranslationBuildCommand {
    pub source: SourceArgs,
    pub translation: PathBuf,
    pub font_config: PathBuf,
    pub font: PathBuf,
    pub output_bin: PathBuf,
    pub output_cue: PathBuf,
    pub preview: PathBuf,
    pub report: PathBuf,
}

pub(super) fn run_memory_card_translation_build(
    command: MemoryCardTranslationBuildCommand,
) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let executable = read_inventory_file(&command.source.bin, &inventory, "/SLPM_624.00;1")?;
    let image_d_container = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_D.BIN;1")?;
    let image_g_container = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_G.BIN;1")?;
    let report = build_memory_card_translation_development(MemoryCardTranslationBuildInputs {
        inventory: &inventory,
        source_bin_path: &command.source.bin,
        executable: &executable,
        image_d_container: &image_d_container,
        image_g_container: &image_g_container,
        translation_path: &command.translation,
        font_config_path: &command.font_config,
        font_path: &command.font,
        output_bin_path: &command.output_bin,
        output_cue_path: &command.output_cue,
        preview_path: &command.preview,
    })?;
    write_json_file(&command.report, &report)?;
    println!(
        "built pre-human-review memory-card development image with {} messages and {} Hangul glyphs across {} changed raw sectors to {} ({}); wrote {}",
        report.translated_messages,
        report.assigned_hangul_glyphs,
        report.raw_sector_plans,
        command.output_bin.display(),
        report.output_bin_sha256,
        command.report.display()
    );
    Ok(())
}
