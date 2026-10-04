use std::path::PathBuf;

use anyhow::Result;
use puyo2_ps2_tool::ranking_translation_build::{
    RankingTranslationBuildInputs, build_ranking_translation_development,
};
use puyo2_ps2_tool::source::{read_inventory_file, write_json_file};

use super::{SourceArgs, inspect};

pub(super) struct RankingTranslationBuildCommand {
    pub source: SourceArgs,
    pub translation: PathBuf,
    pub asset: PathBuf,
    pub output_bin: PathBuf,
    pub output_cue: PathBuf,
    pub preview: PathBuf,
    pub report: PathBuf,
}

pub(super) fn run_ranking_translation_build(command: RankingTranslationBuildCommand) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let executable = read_inventory_file(&command.source.bin, &inventory, "/SLPM_624.00;1")?;
    let image_container = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_G.BIN;1")?;
    let report = build_ranking_translation_development(RankingTranslationBuildInputs {
        inventory: &inventory,
        source_bin_path: &command.source.bin,
        executable: &executable,
        image_container: &image_container,
        translation_path: &command.translation,
        asset_path: &command.asset,
        output_bin_path: &command.output_bin,
        output_cue_path: &command.output_cue,
        preview_path: &command.preview,
    })?;
    write_json_file(&command.report, &report)?;
    println!(
        "built pre-human-review ranking development image across {} changed raw sectors to {} ({}); wrote {}",
        report.raw_sector_plans,
        command.output_bin.display(),
        report.output_bin_sha256,
        command.report.display()
    );
    Ok(())
}
