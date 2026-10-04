use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, ensure};

use super::{TEXT_ROW_INTERIOR_BOTTOM, TEXT_ROW_INTERIOR_TOP, TEXTURE_CELL_PIXELS};
use crate::graphics_translation::raster::read_rgba_surface;
use crate::graphics_translation::{GraphicsTranslationEntry, GraphicsTreatmentProfile};
use crate::story_dialogue_translation::StoryDialogueTranslationEntry;

pub(super) fn physical_text_row_ink_widths(
    story: &StoryDialogueTranslationEntry,
    graphics: &GraphicsTranslationEntry,
) -> Result<BTreeMap<usize, usize>> {
    ensure!(
        graphics.source.container_iso_path == story.source.container_iso_path
            && graphics.source.image_file_record == story.source.image_file_record
            && graphics.source.width == story.source.width
            && graphics.source.height == story.source.height
            && graphics.source.linear_indices_sha256 == story.source.linear_indices_sha256,
        "story and graphics source identities differ: {}",
        story.source.story_surface_id
    );
    ensure!(
        graphics.authoring.regions.len() == 1
            && graphics.authoring.regions[0].treatment_profile
                == GraphicsTreatmentProfile::CutsceneBakedText,
        "story graphics entry has no single baked-text region: {}",
        story.source.story_surface_id
    );
    let region = &graphics.authoring.regions[0];
    let source_text = story
        .authoring
        .segments
        .iter()
        .map(|segment| segment.source_text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    ensure_story_region_uses_story_ledger(
        &story.source.story_surface_id,
        &source_text,
        &region.source_text,
        region.korean_text.as_deref(),
    )?;
    let mask = read_rgba_surface(Path::new(&region.source_pixel_mask.path))?;
    ensure!(
        mask.width == story.source.width as usize && mask.height == story.source.height as usize,
        "story source mask dimensions differ: {}",
        story.source.story_surface_id
    );
    for pixel in mask.pixels.chunks_exact(4) {
        ensure!(
            pixel[0] == pixel[1]
                && pixel[1] == pixel[2]
                && matches!(pixel[0], 0 | u8::MAX)
                && pixel[3] == u8::MAX,
            "story source mask is not binary grayscale: {}",
            story.source.story_surface_id
        );
    }

    let text_rows =
        physical_text_row_ink_widths_from_pixels(mask.width, mask.height, &mask.pixels)?;
    let ledger_rows = story
        .authoring
        .segments
        .iter()
        .map(|segment| segment.source_text.split('\n').count())
        .sum::<usize>();
    ensure!(
        text_rows.len() == ledger_rows,
        "story source mask text rows differ from ledger rows: {} ({} mask rows, {} ledger rows)",
        story.source.story_surface_id,
        text_rows.len(),
        ledger_rows
    );
    Ok(text_rows)
}

fn ensure_story_region_uses_story_ledger(
    story_surface_id: &str,
    story_source_text: &str,
    region_source_text: &str,
    region_korean_text: Option<&str>,
) -> Result<()> {
    ensure!(
        region_source_text == story_source_text,
        "story graphics source text differs from the split ledger: {story_surface_id}"
    );
    ensure!(
        region_korean_text.is_none(),
        "story graphics region duplicates Korean wording owned by the split ledger: {story_surface_id}"
    );
    Ok(())
}

#[cfg(test)]
pub(super) fn ensure_story_region_uses_story_ledger_for_test(
    story_source_text: &str,
    region_source_text: &str,
    region_korean_text: Option<&str>,
) -> Result<()> {
    ensure_story_region_uses_story_ledger(
        "story-dialogue/test",
        story_source_text,
        region_source_text,
        region_korean_text,
    )
}

pub(super) fn physical_text_row_ink_widths_from_pixels(
    width: usize,
    height: usize,
    pixels: &[u8],
) -> Result<BTreeMap<usize, usize>> {
    ensure!(
        pixels.len() == width * height * 4,
        "story source mask dimensions differ from its pixels"
    );
    let mut text_rows = BTreeMap::new();
    for source_row in 0..height.div_ceil(TEXTURE_CELL_PIXELS) {
        let inner_top = source_row * TEXTURE_CELL_PIXELS + TEXT_ROW_INTERIOR_TOP;
        let inner_bottom =
            (source_row * TEXTURE_CELL_PIXELS + TEXT_ROW_INTERIOR_BOTTOM).min(height);
        let right = (inner_top..inner_bottom)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .filter(|(x, y)| pixels[(*y * width + *x) * 4] == u8::MAX)
            .map(|(x, _)| x + 1)
            .max();
        if let Some(right) = right {
            text_rows.insert(source_row, right);
        }
    }
    Ok(text_rows)
}
