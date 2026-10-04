use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use super::StoryDialogueTranslationBundle;
use crate::cpx::read_symbol_bytes;
use crate::graphics_translation::GraphicsTranslationBundle;

mod decoder;
mod parser;
mod source_rows;
mod texture_routes;

use decoder::validate_sequence_command_decoder;
use parser::{
    MESSAGE_WINDOW_COMMAND_WORDS, MESSAGE_WINDOW_OPCODE, parse_command_sizes,
    parse_message_window_commands,
};
use source_rows::physical_text_row_ink_widths;
#[cfg(test)]
use source_rows::physical_text_row_ink_widths_from_pixels;
use texture_routes::validate_story_dialogue_texture_route;

const SEQUENCE_COMMAND_SIZE_SYMBOL: &str = "SeqComSize";
const LAST_STORY_SEQUENCE: usize = 0x49;
const TEXTURE_CELL_PIXELS: usize = 24;
const TEXT_ROW_INTERIOR_TOP: usize = 6;
const TEXT_ROW_INTERIOR_BOTTOM: usize = 19;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoryDialogueSequenceLayoutValidation {
    pub sequence_streams: usize,
    pub message_window_commands: usize,
    pub source_rectangle_rows: usize,
    pub distinct_source_rows: usize,
    pub repeated_source_row_consumptions: usize,
    pub ledger_rows_bound_to_message_windows: usize,
    pub ledger_rows_without_message_windows: usize,
    pub message_window_rows_without_ledger_text: usize,
    pub source_rows_exceeding_maximum_window_width: usize,
    pub dialogue_texture_routes: usize,
    pub message_commands_using_dialogue_texture: usize,
    pub static_pattern_records: usize,
    pub static_patterns_using_dialogue_texture: usize,
    pub minimum_window_width_cells: usize,
    pub maximum_window_width_cells: usize,
}

pub(super) struct StoryDialogueSequenceSurfaceLayout {
    pub(super) consumer_sequence: usize,
    pub(super) physical_text_rows: Vec<usize>,
    pub(super) source_ink_width_pixels_by_row: BTreeMap<usize, usize>,
    pub(super) window_width_pixels_by_row: BTreeMap<usize, StoryDialogueRowWindowWidths>,
    message_window_commands: usize,
    source_rectangle_rows: usize,
    repeated_source_row_consumptions: usize,
    static_pattern_records: usize,
    static_patterns_using_dialogue_texture: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StoryDialogueRowWindowWidths {
    pub(super) minimum: usize,
    pub(super) maximum: usize,
}

pub fn validate_story_dialogue_sequence_layout(
    executable: &[u8],
    bundle: &StoryDialogueTranslationBundle,
    graphics: &GraphicsTranslationBundle,
) -> Result<StoryDialogueSequenceLayoutValidation> {
    let layouts = collect_story_dialogue_sequence_layouts(executable, bundle, graphics)?;
    let mut validation = StoryDialogueSequenceLayoutValidation {
        sequence_streams: layouts.len(),
        message_window_commands: 0,
        source_rectangle_rows: 0,
        distinct_source_rows: 0,
        repeated_source_row_consumptions: 0,
        ledger_rows_bound_to_message_windows: 0,
        ledger_rows_without_message_windows: 0,
        message_window_rows_without_ledger_text: 0,
        source_rows_exceeding_maximum_window_width: 0,
        dialogue_texture_routes: 0,
        message_commands_using_dialogue_texture: 0,
        static_pattern_records: 0,
        static_patterns_using_dialogue_texture: 0,
        minimum_window_width_cells: usize::MAX,
        maximum_window_width_cells: 0,
    };

    for layout in layouts {
        validation.message_window_commands += layout.message_window_commands;
        validation.source_rectangle_rows += layout.source_rectangle_rows;
        validation.distinct_source_rows += layout.window_width_pixels_by_row.len();
        validation.repeated_source_row_consumptions += layout.repeated_source_row_consumptions;
        validation.dialogue_texture_routes += 1;
        validation.message_commands_using_dialogue_texture += layout.message_window_commands;
        validation.static_pattern_records += layout.static_pattern_records;
        validation.static_patterns_using_dialogue_texture +=
            layout.static_patterns_using_dialogue_texture;
        for source_row in &layout.physical_text_rows {
            if layout.window_width_pixels_by_row.contains_key(source_row) {
                validation.ledger_rows_bound_to_message_windows += 1;
            } else {
                validation.ledger_rows_without_message_windows += 1;
            }
        }
        validation.message_window_rows_without_ledger_text += layout
            .window_width_pixels_by_row
            .keys()
            .filter(|source_row| !layout.physical_text_rows.contains(source_row))
            .count();
        validation.source_rows_exceeding_maximum_window_width += layout
            .source_ink_width_pixels_by_row
            .iter()
            .filter(|(source_row, source_width)| {
                layout
                    .window_width_pixels_by_row
                    .get(source_row)
                    .is_some_and(|window| **source_width > window.maximum)
            })
            .count();
        for widths in layout.window_width_pixels_by_row.values() {
            let minimum_width_cells = widths.minimum / TEXTURE_CELL_PIXELS;
            let maximum_width_cells = widths.maximum / TEXTURE_CELL_PIXELS;
            validation.minimum_window_width_cells = validation
                .minimum_window_width_cells
                .min(minimum_width_cells);
            validation.maximum_window_width_cells = validation
                .maximum_window_width_cells
                .max(maximum_width_cells);
        }
    }

    ensure!(
        validation.minimum_window_width_cells != usize::MAX,
        "story sequences contain no message-window widths"
    );
    Ok(validation)
}

pub(super) fn collect_story_dialogue_sequence_layouts(
    executable: &[u8],
    bundle: &StoryDialogueTranslationBundle,
    graphics: &GraphicsTranslationBundle,
) -> Result<Vec<StoryDialogueSequenceSurfaceLayout>> {
    let file = object::File::parse(executable).context("failed to parse story executable")?;
    validate_sequence_command_decoder(&file)?;
    let (_, command_size_bytes) = read_symbol_bytes(&file, SEQUENCE_COMMAND_SIZE_SYMBOL)?;
    let command_sizes = parse_command_sizes(&command_size_bytes)?;
    ensure!(
        command_sizes
            .get(MESSAGE_WINDOW_OPCODE)
            .is_some_and(|size| *size == MESSAGE_WINDOW_COMMAND_WORDS),
        "story message-window command size changed"
    );
    ensure!(
        bundle.entries.len() == LAST_STORY_SEQUENCE + 1,
        "story ledger does not cover every DEMO00..49 sequence"
    );

    let mut layouts = Vec::with_capacity(bundle.entries.len());

    for (expected_sequence, entry) in bundle.entries.iter().enumerate() {
        ensure!(
            entry.source.consumer_sequence == expected_sequence,
            "story ledger sequence order changed at {}",
            entry.source.story_surface_id
        );
        let symbol = format!("DEMO{expected_sequence:02X}");
        let (_, sequence_bytes) = read_symbol_bytes(&file, &symbol)?;
        let commands = parse_message_window_commands(&symbol, &sequence_bytes, &command_sizes)?;
        ensure!(
            !commands.is_empty(),
            "story sequence has no message-window command: {symbol}"
        );
        let message_window_commands = commands.len();
        let texture_route = validate_story_dialogue_texture_route(&file, entry, &commands)?;
        ensure!(
            texture_route.message_commands == message_window_commands,
            "story dialogue texture route command count changed"
        );

        let mut row_widths = BTreeMap::<usize, Vec<usize>>::new();
        let mut source_rectangle_rows = 0;
        for command in commands {
            let right = command
                .width_cells
                .checked_mul(TEXTURE_CELL_PIXELS)
                .context("story message-window width overflow")?;
            let bottom_row = command
                .source_row
                .checked_add(command.height_cells)
                .context("story message-window source-row overflow")?;
            let bottom = bottom_row
                .checked_mul(TEXTURE_CELL_PIXELS)
                .context("story message-window height overflow")?;
            ensure!(
                command.width_cells > 0
                    && command.height_cells > 0
                    && right <= entry.source.width as usize
                    && bottom <= entry.source.height as usize,
                "story message-window rectangle exceeds {}: row {}, {}x{} cells",
                entry.source.story_surface_id,
                command.source_row,
                command.width_cells,
                command.height_cells
            );
            source_rectangle_rows += command.height_cells;
            for source_row in command.source_row..bottom_row {
                row_widths
                    .entry(source_row)
                    .or_default()
                    .push(command.width_cells);
            }
        }

        let graphics_entry = graphics
            .entries
            .iter()
            .find(|candidate| candidate.source.source_surface_id == entry.source.source_surface_id)
            .with_context(|| {
                format!(
                    "story source has no graphics entry: {}",
                    entry.source.story_surface_id
                )
            })?;
        let source_ink_width_pixels_by_row = physical_text_row_ink_widths(entry, graphics_entry)?;
        let text_rows = source_ink_width_pixels_by_row.keys().copied().collect();
        let repeated_source_row_consumptions = row_widths
            .values()
            .map(|widths| widths.len() - 1)
            .sum::<usize>();
        let window_width_pixels_by_row = row_widths
            .into_iter()
            .map(|(source_row, widths)| (source_row, row_window_widths(&widths)))
            .collect();
        layouts.push(StoryDialogueSequenceSurfaceLayout {
            consumer_sequence: expected_sequence,
            physical_text_rows: text_rows,
            source_ink_width_pixels_by_row,
            window_width_pixels_by_row,
            message_window_commands,
            source_rectangle_rows,
            repeated_source_row_consumptions,
            static_pattern_records: texture_route.static_pattern_records,
            static_patterns_using_dialogue_texture: texture_route
                .static_patterns_using_dialogue_texture,
        });
    }
    Ok(layouts)
}

fn row_window_widths(width_cells: &[usize]) -> StoryDialogueRowWindowWidths {
    StoryDialogueRowWindowWidths {
        minimum: width_cells
            .iter()
            .copied()
            .min()
            .expect("message-window row has at least one width")
            * TEXTURE_CELL_PIXELS,
        maximum: width_cells
            .iter()
            .copied()
            .max()
            .expect("message-window row has at least one width")
            * TEXTURE_CELL_PIXELS,
    }
}

#[cfg(test)]
pub(super) type ParsedMessageWindowCommandForTest = (usize, usize, usize, usize, usize);

#[cfg(test)]
pub(super) fn parse_message_window_commands_for_test(
    source: &[u8],
    command_sizes: &[usize],
) -> Result<Vec<ParsedMessageWindowCommandForTest>> {
    Ok(
        parse_message_window_commands("DEMOtest", source, command_sizes)?
            .into_iter()
            .map(|command| {
                (
                    command.dialogue_texture_index,
                    command.window_texture_index,
                    command.source_row,
                    command.width_cells,
                    command.height_cells,
                )
            })
            .collect(),
    )
}

#[cfg(test)]
pub(super) fn physical_text_rows_for_test(
    width: usize,
    height: usize,
    pixels: &[u8],
) -> Result<Vec<usize>> {
    Ok(
        physical_text_row_ink_widths_from_pixels(width, height, pixels)?
            .into_keys()
            .collect(),
    )
}

#[cfg(test)]
pub(super) fn ensure_story_region_uses_story_ledger_for_test(
    story_source_text: &str,
    region_source_text: &str,
    region_korean_text: Option<&str>,
) -> Result<()> {
    source_rows::ensure_story_region_uses_story_ledger_for_test(
        story_source_text,
        region_source_text,
        region_korean_text,
    )
}

#[cfg(test)]
pub(super) fn physical_text_row_widths_for_test(
    width: usize,
    height: usize,
    pixels: &[u8],
) -> Result<BTreeMap<usize, usize>> {
    physical_text_row_ink_widths_from_pixels(width, height, pixels)
}

#[cfg(test)]
pub(super) fn row_window_widths_for_test(width_cells: &[usize]) -> (usize, usize) {
    let widths = row_window_widths(width_cells);
    (widths.minimum, widths.maximum)
}
