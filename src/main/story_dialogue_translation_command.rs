use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::{
    GraphicsTranslationBundle, GraphicsTranslationInputs, load_graphics_translation_set,
    validate_graphics_translation_asset_files, validate_graphics_translation_bundle,
};
use puyo2_ps2_tool::source::{
    DiscInventory, find_inventory_file, read_inventory_file, read_json_file,
};
use puyo2_ps2_tool::story_dialogue_translation::{
    StoryDialogueTextureCellConsumerValidation, StoryDialogueTranslationBundle,
    compose_story_dialogue_translation_bundle, load_story_dialogue_translation_set,
    measure_story_dialogue_rendered_layout, validate_story_dialogue_sequence_layout,
    validate_story_dialogue_texture_cell_consumer, validate_story_dialogue_translation_bundle,
    write_new_story_dialogue_translation_set,
};
use puyo2_ps2_tool::translation_targets::LinkedTextureTarget;
use serde::Deserialize;

use super::{SourceArgs, inspect};

pub(super) struct StoryDialogueTranslationCommand {
    pub source: SourceArgs,
    pub target_map: PathBuf,
    pub graphics_translation: PathBuf,
    pub story_translation: PathBuf,
    pub font_config: Option<PathBuf>,
    pub font: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct GraphicsTargetMap {
    schema: u32,
    source_bin_sha256: String,
    executable_iso_path: String,
    executable_sha256: String,
    linked_texture_targets: Vec<LinkedTextureTarget>,
}

impl GraphicsTargetMap {
    fn inputs(&self) -> GraphicsTranslationInputs<'_> {
        GraphicsTranslationInputs {
            source_bin_sha256: &self.source_bin_sha256,
            executable_iso_path: &self.executable_iso_path,
            executable_sha256: &self.executable_sha256,
            linked_texture_targets: &self.linked_texture_targets,
        }
    }
}

struct VerifiedStoryDialogueInputs {
    graphics: GraphicsTranslationBundle,
    texture_cell_consumer: StoryDialogueTextureCellConsumerValidation,
    executable: Vec<u8>,
}

pub(super) fn run_story_dialogue_translation_init(
    command: StoryDialogueTranslationCommand,
) -> Result<()> {
    let inputs = load_verified_story_dialogue_inputs(&command)?;
    let bundle = compose_story_dialogue_translation_bundle(&inputs.graphics)?;
    write_new_story_dialogue_translation_set(&command.story_translation, &bundle)?;
    println!(
        "initialized {} exact-source story dialogue surfaces in DEMOTexTbl consumer order with a typed {}x{}-pixel texture cell consumer as a partitioned set at {}",
        bundle.population.dialogue_surfaces,
        inputs.texture_cell_consumer.cell_width_pixels,
        inputs.texture_cell_consumer.cell_height_pixels,
        command.story_translation.display()
    );
    Ok(())
}

pub(super) fn run_story_dialogue_translation_check(
    command: StoryDialogueTranslationCommand,
) -> Result<()> {
    let inputs = load_verified_story_dialogue_inputs(&command)?;
    let bundle: StoryDialogueTranslationBundle =
        load_story_dialogue_translation_set(&command.story_translation)?;
    let validation = validate_story_dialogue_translation_bundle(&bundle, &inputs.graphics)?;
    let sequence_layout =
        validate_story_dialogue_sequence_layout(&inputs.executable, &bundle, &inputs.graphics)?;
    let rendered_layout = match (&command.font_config, &command.font) {
        (Some(font_config), Some(font)) => Some(measure_story_dialogue_rendered_layout(
            &inputs.executable,
            &bundle,
            &inputs.graphics,
            font_config,
            font,
        )?),
        (None, None) => None,
        _ => anyhow::bail!("story dialogue font and font configuration must be supplied together"),
    };
    println!(
        "validated {} story dialogue surfaces with a typed {}x{}-pixel texture cell consumer: {} need transcription, {} need review, {} await human review, {} complete; {} release-candidate eligible; {} transcribed segments with {} source-bound Korean layout lines, {} source characters, {} Korean characters, and {} sibling reference links; {} DEMO streams contain {} message-window commands covering {} distinct source rows, binding {} ledger rows while leaving {} dormant ledger rows outside that consumer and {} consumed rows without ledger text; {} source rows extend beyond their largest partial message-window rectangle; {} exact dialogue-texture routes carry all {} message commands while {}/{} static patterns select the dialogue texture",
        validation.dialogue_surfaces,
        inputs.texture_cell_consumer.cell_width_pixels,
        inputs.texture_cell_consumer.cell_height_pixels,
        validation.needs_transcription,
        validation.needs_review,
        validation.needs_human_review,
        validation.complete,
        validation.release_candidate_eligible,
        validation.transcribed_segments,
        validation.source_bound_layout_lines,
        validation.source_characters,
        validation.korean_characters,
        validation.sibling_reference_links,
        sequence_layout.sequence_streams,
        sequence_layout.message_window_commands,
        sequence_layout.distinct_source_rows,
        sequence_layout.ledger_rows_bound_to_message_windows,
        sequence_layout.ledger_rows_without_message_windows,
        sequence_layout.message_window_rows_without_ledger_text,
        sequence_layout.source_rows_exceeding_maximum_window_width,
        sequence_layout.dialogue_texture_routes,
        sequence_layout.message_commands_using_dialogue_texture,
        sequence_layout.static_patterns_using_dialogue_texture,
        sequence_layout.static_pattern_records,
    );
    if let Some(layout) = rendered_layout {
        println!(
            "measured {} story rows with {} at {:.3}px and a {}px outline: {} rows fit their exact-source ink envelopes, {} overflow, {} rows are outside the message-window consumer, {} rows are bound to it, and {} rows exceed the 24px texture height",
            layout.layout_rows,
            layout.font_id,
            layout.font_pixel_size_milli as f32 / 1000.0,
            layout.outline_radius_pixels,
            layout.fitting_rows,
            layout.overflowing_rows,
            layout.rows_outside_message_window_consumer,
            layout.message_window_rows,
            layout.rows_exceeding_texture_height,
        );
        for row in &layout.overflows {
            println!(
                "overflow {} {}/row {} texture row {}: {:?} needs {}px within a {}px source envelope (+{}px)",
                row.story_surface_id,
                row.segment_id,
                row.segment_row,
                row.texture_row,
                row.korean_text,
                row.rendered_width_pixels,
                row.source_envelope_width_pixels,
                row.overflow_pixels,
            );
        }
        ensure!(
            layout.overflowing_rows == 0 && layout.rows_exceeding_texture_height == 0,
            "story Korean rendered layout exceeds an exact-source row envelope"
        );
    }
    Ok(())
}

fn load_verified_story_dialogue_inputs(
    command: &StoryDialogueTranslationCommand,
) -> Result<VerifiedStoryDialogueInputs> {
    let inventory = inspect(&command.source)?;
    let target_map: GraphicsTargetMap = read_json_file(&command.target_map)?;
    verify_target_map_identity(&inventory, &target_map)?;
    let executable = read_inventory_file(
        &command.source.bin,
        &inventory,
        &target_map.executable_iso_path,
    )?;
    let texture_cell_consumer = validate_story_dialogue_texture_cell_consumer(&executable)?;
    let graphics = load_graphics_translation_set(&command.graphics_translation)?;
    validate_graphics_translation_bundle(&graphics, target_map.inputs())?;
    validate_graphics_translation_asset_files(&graphics)?;
    Ok(VerifiedStoryDialogueInputs {
        graphics,
        texture_cell_consumer,
        executable,
    })
}

fn verify_target_map_identity(
    inventory: &DiscInventory,
    target_map: &GraphicsTargetMap,
) -> Result<()> {
    ensure!(
        target_map.schema == 1,
        "unsupported translation target schema"
    );
    ensure!(
        target_map.source_bin_sha256 == inventory.source.bin.sha256,
        "story graphics target map differs from the verified source BIN"
    );
    let executable_sha256 = find_inventory_file(inventory, &target_map.executable_iso_path)?
        .sha256
        .as_deref()
        .with_context(|| {
            format!(
                "executable inventory entry has no SHA-256: {}",
                target_map.executable_iso_path
            )
        })?;
    ensure!(
        target_map.executable_sha256 == executable_sha256,
        "story graphics target map differs from the verified executable"
    );
    ensure!(
        !target_map.linked_texture_targets.is_empty(),
        "story graphics target map has no linked texture targets"
    );
    Ok(())
}
