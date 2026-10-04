use anyhow::{Result, ensure};

use super::parser::MessageWindowCommand;
use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::parse_load_records;
use crate::story_dialogue_translation::StoryDialogueTranslationEntry;
use crate::texture_pattern::parse_pattern_records;

pub(super) struct StoryDialogueTextureRoute {
    pub(super) message_commands: usize,
    pub(super) static_pattern_records: usize,
    pub(super) static_patterns_using_dialogue_texture: usize,
}

pub(super) fn validate_story_dialogue_texture_route(
    file: &object::File<'_>,
    entry: &StoryDialogueTranslationEntry,
    commands: &[MessageWindowCommand],
) -> Result<StoryDialogueTextureRoute> {
    let load_table_symbol = &entry.source.consumer_table;
    let (_, load_table_bytes) = read_symbol_bytes(file, load_table_symbol)?;
    let loads = parse_load_records(&load_table_bytes)?;
    let images = loads
        .iter()
        .filter(|record| record.operation == 1)
        .collect::<Vec<_>>();
    let palettes = loads
        .iter()
        .filter(|record| record.operation == 2)
        .collect::<Vec<_>>();
    ensure!(
        !images.is_empty() && images.len() == palettes.len(),
        "story texture load table is not a paired image/palette set: {load_table_symbol}"
    );
    let matching_indices = images
        .iter()
        .enumerate()
        .filter(|(_, record)| record.file_record == entry.source.image_file_record)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    ensure!(
        matching_indices.len() == 1,
        "story dialogue image does not appear exactly once in {load_table_symbol}"
    );
    let dialogue_image_load_index = matching_indices[0];

    let pattern_symbol = format!("DEMOPtn{:02X}", entry.source.consumer_sequence);
    let (_, pattern_bytes) = read_symbol_bytes(file, &pattern_symbol)?;
    let patterns = parse_pattern_records(&pattern_symbol, &pattern_bytes)?;
    ensure!(
        !patterns.is_empty()
            && patterns
                .iter()
                .all(|pattern| pattern.image_load_index < images.len()),
        "story static pattern references an out-of-range image in {pattern_symbol}"
    );
    let static_patterns_using_dialogue_texture = patterns
        .iter()
        .filter(|pattern| pattern.image_load_index == dialogue_image_load_index)
        .count();
    ensure!(
        static_patterns_using_dialogue_texture == 0,
        "story dialogue texture is also selected by a static pattern: {pattern_symbol}"
    );

    ensure!(
        commands.iter().all(|command| {
            command.dialogue_texture_index == dialogue_image_load_index
                && command.window_texture_index < images.len()
                && command.window_texture_index != dialogue_image_load_index
        }),
        "story message command does not select the exact dialogue texture: {}",
        entry.source.story_surface_id
    );
    Ok(StoryDialogueTextureRoute {
        message_commands: commands.len(),
        static_pattern_records: patterns.len(),
        static_patterns_using_dialogue_texture,
    })
}
