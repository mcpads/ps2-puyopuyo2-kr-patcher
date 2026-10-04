use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use super::StoryDialogueTranslationBundle;
use super::font::{
    TEXTURE_ROW_HEIGHT_PIXELS, load_story_dialogue_font, rasterize_story_dialogue_line,
};
use super::protected_english::{load_story_protected_english_mask, plan_protected_english_row};
use super::sequence::collect_story_dialogue_sequence_layouts;
use crate::graphics_translation::GraphicsTranslationBundle;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoryDialogueRenderedLayoutValidation {
    pub font_id: String,
    pub font_sha256: String,
    pub font_pixel_size_milli: u32,
    pub font_coverage_threshold: u8,
    pub outline_radius_pixels: usize,
    pub layout_rows: usize,
    pub message_window_rows: usize,
    pub rows_outside_message_window_consumer: usize,
    pub fitting_rows: usize,
    pub overflowing_rows: usize,
    pub rows_exceeding_texture_height: usize,
    pub narrowest_remaining_width_pixels: usize,
    pub maximum_width_overflow_pixels: usize,
    pub overflows: Vec<StoryDialogueRowOverflow>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoryDialogueRowOverflow {
    pub story_surface_id: String,
    pub segment_id: String,
    pub segment_row: usize,
    pub texture_row: usize,
    pub korean_text: String,
    pub rendered_width_pixels: usize,
    pub source_envelope_width_pixels: usize,
    pub overflow_pixels: usize,
}

pub fn measure_story_dialogue_rendered_layout(
    executable: &[u8],
    bundle: &StoryDialogueTranslationBundle,
    graphics: &GraphicsTranslationBundle,
    font_config_path: &Path,
    font_path: &Path,
) -> Result<StoryDialogueRenderedLayoutValidation> {
    let font = load_story_dialogue_font(font_config_path, font_path)?;
    let config = &font.config;
    let layouts = collect_story_dialogue_sequence_layouts(executable, bundle, graphics)?;
    ensure!(
        layouts.len() == bundle.entries.len(),
        "story sequence layout population differs from the translation ledger"
    );

    let mut validation = StoryDialogueRenderedLayoutValidation {
        font_id: config.id.clone(),
        font_sha256: config.sha256.clone(),
        font_pixel_size_milli: (config.rasterization.pixel_size * 1000.0).round() as u32,
        font_coverage_threshold: config.rasterization.coverage_threshold,
        outline_radius_pixels: config.rasterization.outline_radius,
        layout_rows: 0,
        message_window_rows: 0,
        rows_outside_message_window_consumer: 0,
        fitting_rows: 0,
        overflowing_rows: 0,
        rows_exceeding_texture_height: 0,
        narrowest_remaining_width_pixels: usize::MAX,
        maximum_width_overflow_pixels: 0,
        overflows: Vec::new(),
    };

    for (entry, layout) in bundle.entries.iter().zip(layouts) {
        ensure!(
            entry.source.consumer_sequence == layout.consumer_sequence,
            "story sequence layout order differs from the translation ledger"
        );
        let graphics_entry = graphics
            .entries
            .iter()
            .find(|candidate| candidate.source.source_surface_id == entry.source.source_surface_id)
            .with_context(|| {
                format!(
                    "story layout has no graphics entry: {}",
                    entry.source.story_surface_id
                )
            })?;
        let protected_english_mask = load_story_protected_english_mask(
            graphics_entry,
            usize::try_from(entry.source.width)?,
            usize::try_from(entry.source.height)?,
        )?;
        let surface_width = usize::try_from(entry.source.width)?;
        ensure!(
            protected_english_mask
                .iter()
                .enumerate()
                .filter(|(_, selected)| **selected)
                .all(|(pixel, _)| layout
                    .physical_text_rows
                    .contains(&(pixel / surface_width / TEXTURE_ROW_HEIGHT_PIXELS))),
            "story English mask selects a row outside the dialogue consumer"
        );
        let rows = entry
            .authoring
            .segments
            .iter()
            .flat_map(|segment| {
                segment
                    .source_text
                    .split('\n')
                    .zip(segment.korean_text.split('\n'))
                    .enumerate()
                    .map(move |(row_index, (source_text, korean_text))| {
                        (segment, row_index, source_text, korean_text)
                    })
            })
            .collect::<Vec<_>>();
        ensure!(
            rows.len() == layout.physical_text_rows.len(),
            "story Korean rows differ from physical source rows: {}",
            entry.source.story_surface_id
        );

        for ((segment, segment_row, source_text, korean_text), texture_row) in
            rows.into_iter().zip(layout.physical_text_rows)
        {
            validation.layout_rows += 1;
            if layout.window_width_pixels_by_row.contains_key(&texture_row) {
                validation.message_window_rows += 1;
            } else {
                validation.rows_outside_message_window_consumer += 1;
            }
            let source_envelope_width_pixels = *layout
                .source_ink_width_pixels_by_row
                .get(&texture_row)
                .context("physical story row has no source ink envelope")?;
            let plan = plan_protected_english_row(
                source_text,
                korean_text,
                &protected_english_mask,
                surface_width,
                usize::try_from(entry.source.height)?,
                texture_row,
            )?;
            let composed_source_envelope_width_pixels =
                source_envelope_width_pixels.max(plan.protected_right_edge());
            let mut rendered_right_edge = plan.protected_right_edge();
            let mut row_overflow_pixels = 0_usize;
            let mut row_remaining_pixels = usize::MAX;
            let mut rendered_height = 0_usize;
            for slot in plan.fragment_slots(source_envelope_width_pixels) {
                if !slot
                    .text
                    .chars()
                    .any(|character| !character.is_whitespace())
                {
                    continue;
                }
                let rendered = rasterize_story_dialogue_line(&font, slot.text)?;
                let right = slot
                    .x
                    .checked_add(rendered.width)
                    .context("story layout fragment width overflow")?;
                rendered_right_edge = rendered_right_edge.max(right);
                rendered_height = rendered_height.max(rendered.height);
                if right <= slot.right_limit {
                    row_remaining_pixels = row_remaining_pixels.min(slot.right_limit - right);
                } else {
                    row_overflow_pixels = row_overflow_pixels.max(right - slot.right_limit);
                }
            }
            if rendered_height > TEXTURE_ROW_HEIGHT_PIXELS {
                validation.rows_exceeding_texture_height += 1;
            }
            if row_overflow_pixels == 0 {
                validation.fitting_rows += 1;
                if row_remaining_pixels == usize::MAX {
                    row_remaining_pixels =
                        composed_source_envelope_width_pixels.saturating_sub(rendered_right_edge);
                }
                validation.narrowest_remaining_width_pixels = validation
                    .narrowest_remaining_width_pixels
                    .min(row_remaining_pixels);
                continue;
            }
            validation.overflowing_rows += 1;
            validation.maximum_width_overflow_pixels = validation
                .maximum_width_overflow_pixels
                .max(row_overflow_pixels);
            validation.overflows.push(StoryDialogueRowOverflow {
                story_surface_id: entry.source.story_surface_id.clone(),
                segment_id: segment.segment_id.clone(),
                segment_row: segment_row + 1,
                texture_row,
                korean_text: korean_text.to_string(),
                rendered_width_pixels: rendered_right_edge,
                source_envelope_width_pixels: composed_source_envelope_width_pixels,
                overflow_pixels: row_overflow_pixels,
            });
        }
    }
    if validation.narrowest_remaining_width_pixels == usize::MAX {
        validation.narrowest_remaining_width_pixels = 0;
    }
    validation.overflows.sort_by_key(|row| {
        (
            std::cmp::Reverse(row.overflow_pixels),
            row.story_surface_id.clone(),
            row.segment_id.clone(),
            row.segment_row,
        )
    });
    Ok(validation)
}
