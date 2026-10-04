use anyhow::{Context, Result, ensure};

use super::super::font::{RasterizedStoryDialogueLine, StoryDialogueGlyphPixel};

pub(super) fn clear_source_text(
    replacement_indices: &mut [u8],
    source_text_mask: &[bool],
    transparent_index: u8,
) -> Result<()> {
    ensure!(
        replacement_indices.len() == source_text_mask.len(),
        "story source mask dimensions differ from its indexed texture"
    );
    for (index, selected) in replacement_indices.iter_mut().zip(source_text_mask) {
        if *selected {
            *index = transparent_index;
        }
    }
    Ok(())
}

pub(super) struct StoryDialogueLinePlacement<'a> {
    pub(super) surface_width: usize,
    pub(super) surface_height: usize,
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) line: &'a RasterizedStoryDialogueLine,
    pub(super) outline_index: u8,
    pub(super) fill_index: u8,
    pub(super) protected_pixel_mask: &'a [bool],
}

pub(super) fn place_story_dialogue_line(
    replacement_indices: &mut [u8],
    korean_text_mask: &mut [bool],
    placement: StoryDialogueLinePlacement<'_>,
) -> Result<()> {
    ensure!(
        replacement_indices.len() == placement.surface_width * placement.surface_height
            && korean_text_mask.len() == replacement_indices.len()
            && placement.protected_pixel_mask.len() == replacement_indices.len(),
        "story replacement surface dimensions changed"
    );
    let right = placement
        .x
        .checked_add(placement.line.width)
        .context("story line horizontal placement overflow")?;
    let bottom = placement
        .y
        .checked_add(placement.line.height)
        .context("story line vertical placement overflow")?;
    ensure!(
        right <= placement.surface_width && bottom <= placement.surface_height,
        "story Korean line exceeds its texture surface"
    );
    for line_y in 0..placement.line.height {
        for line_x in 0..placement.line.width {
            let role = placement.line.pixels[line_y * placement.line.width + line_x];
            if role == StoryDialogueGlyphPixel::Transparent {
                continue;
            }
            let destination =
                (placement.y + line_y) * placement.surface_width + placement.x + line_x;
            ensure!(
                !placement.protected_pixel_mask[destination],
                "story Korean text overlaps a protected source pixel"
            );
            replacement_indices[destination] = match role {
                StoryDialogueGlyphPixel::Transparent => unreachable!(),
                StoryDialogueGlyphPixel::Outline => placement.outline_index,
                StoryDialogueGlyphPixel::Fill => placement.fill_index,
            };
            korean_text_mask[destination] = true;
        }
    }
    Ok(())
}

pub(super) fn verify_story_replacement_pixels(
    source_indices: &[u8],
    replacement_indices: &[u8],
    source_text_mask: &[bool],
    korean_text_mask: &[bool],
    protected_english_mask: &[bool],
    transparent_index: u8,
) -> Result<StoryReplacementPixelCounts> {
    ensure!(
        source_indices.len() == replacement_indices.len()
            && source_indices.len() == source_text_mask.len()
            && source_indices.len() == korean_text_mask.len()
            && source_indices.len() == protected_english_mask.len(),
        "story replacement verification dimensions changed"
    );
    let mut counts = StoryReplacementPixelCounts::default();
    for pixel in 0..source_indices.len() {
        let editable = source_text_mask[pixel] || korean_text_mask[pixel];
        ensure!(
            !(protected_english_mask[pixel]
                && (source_text_mask[pixel] || korean_text_mask[pixel])),
            "story English protection overlaps editable Japanese or Korean pixels"
        );
        if editable {
            counts.editable_pixels += 1;
        } else {
            counts.protected_pixels += 1;
            if source_indices[pixel] != replacement_indices[pixel] {
                counts.changed_protected_pixels += 1;
            }
        }
        if source_indices[pixel] != replacement_indices[pixel] {
            counts.changed_pixels += 1;
        }
        if protected_english_mask[pixel] {
            counts.protected_english_pixels += 1;
            if source_indices[pixel] != replacement_indices[pixel] {
                counts.changed_protected_english_pixels += 1;
            }
        }
        if source_text_mask[pixel]
            && !korean_text_mask[pixel]
            && replacement_indices[pixel] != transparent_index
        {
            counts.uncleared_source_only_pixels += 1;
        }
    }
    ensure!(
        counts.changed_protected_pixels == 0
            && counts.changed_protected_english_pixels == 0
            && counts.uncleared_source_only_pixels == 0
            && counts.changed_pixels > 0,
        "story replacement changed protected pixels, retained source-only pixels, or changed nothing"
    );
    Ok(counts)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct StoryReplacementPixelCounts {
    pub(super) editable_pixels: usize,
    pub(super) protected_pixels: usize,
    pub(super) protected_english_pixels: usize,
    pub(super) changed_pixels: usize,
    pub(super) changed_protected_pixels: usize,
    pub(super) changed_protected_english_pixels: usize,
    pub(super) uncleared_source_only_pixels: usize,
}
