use std::path::PathBuf;

use anyhow::Result;
use puyo2_ps2_tool::graphics_translation::deterministic_label::{
    DeterministicLabelMasterConfiguration, render_deterministic_label_master,
};
use puyo2_ps2_tool::source::{read_json_file, write_json_file};

pub(super) struct DeterministicLabelMasterCommand {
    pub configuration: PathBuf,
    pub font_configuration: PathBuf,
    pub font: PathBuf,
    pub output: PathBuf,
    pub report: PathBuf,
    pub refresh_existing: bool,
}

pub(super) fn run_deterministic_label_master_render(
    command: DeterministicLabelMasterCommand,
) -> Result<()> {
    let configuration: DeterministicLabelMasterConfiguration =
        read_json_file(&command.configuration)?;
    let report = render_deterministic_label_master(
        &configuration,
        &command.font_configuration,
        &command.font,
        &command.output,
        command.refresh_existing,
    )?;
    write_json_file(&command.report, &report)?;
    println!(
        "rendered deterministic label {} as {}x{} master {} and report {}",
        report.text,
        report.width,
        report.height,
        command.output.display(),
        command.report.display()
    );
    Ok(())
}
