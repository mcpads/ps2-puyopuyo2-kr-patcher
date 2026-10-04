use std::path::PathBuf;

use anyhow::Result;
use puyo2_ps2_tool::source::{read_inventory_file, write_json_file};
use puyo2_ps2_tool::title_translation_build::{
    TitleTranslationBuildInputs, build_title_translation_development,
};

use super::{SourceArgs, inspect};

pub(super) struct TitleTranslationBuildCommand {
    pub source: SourceArgs,
    pub translation: PathBuf,
    pub asset: PathBuf,
    pub output_bin: PathBuf,
    pub output_cue: PathBuf,
    pub preview: PathBuf,
    pub report: PathBuf,
}

pub(super) fn run_title_translation_build(command: TitleTranslationBuildCommand) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let executable = read_inventory_file(&command.source.bin, &inventory, "/SLPM_624.00;1")?;
    let image_container = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_D.BIN;1")?;
    let report = build_title_translation_development(TitleTranslationBuildInputs {
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
        "built pre-human-review title development image across {} changed raw sectors to {} ({}); wrote {}",
        report.raw_sector_plans,
        command.output_bin.display(),
        report.output_bin_sha256,
        command.report.display()
    );
    Ok(())
}
