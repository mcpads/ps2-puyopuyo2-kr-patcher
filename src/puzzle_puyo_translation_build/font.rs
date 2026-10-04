use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use fontdue::{Font, FontSettings};

use crate::cpx::read_symbol_bytes;
use crate::executable_text_translation::ExecutableTextGlyphAssignment;
use crate::font_atlas::{FontGlyphSet, nazo::NazoFontAtlasReport};
use crate::indexed_texture::{
    FileRecord, parse_file_records, parse_load_records, read_decoded_image, read_palette, sha256,
    unswizzle_psmt8_uploaded_as_psmct32,
};

use super::model::{FontConfiguration, RasterizationConfiguration};

const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image2";
const LOAD_TABLE_SYMBOL: &str = "NazoTexTbl";
const FONT_IMAGE_LOAD_INDEX: usize = 25;
const FONT_PALETTE_SLOT: usize = 7;

pub(super) struct PuzzlePuyoFontResources {
    pub(super) image_record: FileRecord,
    pub(super) palette_record_index: usize,
    pub(super) palette: Vec<u8>,
    pub(super) linear_indices: Vec<u8>,
}

pub(super) fn load_font(config: &FontConfiguration, path: &Path) -> Result<Font> {
    ensure!(
        config.schema == 1,
        "unsupported Puzzle Puyo font config schema"
    );
    ensure!(
        path.file_name().and_then(|name| name.to_str()) == Some(&config.expected_filename),
        "Puzzle Puyo font filename differs from its fixed configuration"
    );
    ensure!(
        config.rasterization.pixel_size.is_finite()
            && config.rasterization.pixel_size > 0.0
            && config.rasterization.pixel_size <= 32.0,
        "Puzzle Puyo font pixel size does not fit the 32x32 cell"
    );
    ensure!(
        !config.id.is_empty()
            && !config.license.is_empty()
            && !config.upstream_repository.is_empty(),
        "Puzzle Puyo font provenance is incomplete"
    );
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    ensure!(
        sha256(&bytes) == config.sha256,
        "Puzzle Puyo font SHA-256 differs from its fixed configuration"
    );
    Font::from_bytes(bytes, FontSettings::default())
        .map_err(|error| anyhow::anyhow!("failed to parse Puzzle Puyo font: {error}"))
}

pub(super) fn load_font_resources(
    executable: &[u8],
    container: &[u8],
    report: &NazoFontAtlasReport,
) -> Result<PuzzlePuyoFontResources> {
    let file = object::File::parse(executable).context("failed to parse executable")?;
    let (_, load_bytes) = read_symbol_bytes(&file, LOAD_TABLE_SYMBOL)?;
    let loads = parse_load_records(&load_bytes)?;
    let image_count = loads
        .iter()
        .take_while(|record| record.operation == 1)
        .count();
    ensure!(
        image_count == 29 && loads.len() == 45,
        "Puzzle Puyo texture-load population changed"
    );
    let image_load = loads[FONT_IMAGE_LOAD_INDEX];
    let palette_load = loads[image_count + FONT_PALETTE_SLOT];
    let (_, file_list_bytes) = read_symbol_bytes(&file, IMAGE_FILE_LIST_SYMBOL)?;
    let records = parse_file_records(&file_list_bytes)?;
    let image_record = *records
        .get(image_load.file_record)
        .context("Puzzle Puyo font image record is out of range")?;
    let palette_record = *records
        .get(palette_load.file_record)
        .context("Puzzle Puyo font palette record is out of range")?;
    ensure!(
        image_load.file_record == report.atlas.image_file_record
            && palette_load.file_record == report.atlas.palette_file_record
            && image_record.width == 512
            && image_record.height == 256,
        "Puzzle Puyo font resources differ from the consumer-linked report"
    );
    let decoded = read_decoded_image(container, image_record)?;
    let linear_indices = unswizzle_psmt8_uploaded_as_psmct32(&decoded, 512, 256)?;
    let palette = read_palette(container, palette_record, palette_load.flags & 2 != 0)?;
    Ok(PuzzlePuyoFontResources {
        image_record,
        palette_record_index: palette_load.file_record,
        palette,
        linear_indices,
    })
}

pub(super) fn choose_background_index(
    indices: &[u8],
    report: &NazoFontAtlasReport,
    assignments: &[ExecutableTextGlyphAssignment],
) -> Result<u8> {
    let glyphs = report
        .fullwidth
        .glyphs
        .iter()
        .map(|glyph| (glyph.index, glyph))
        .collect::<BTreeMap<_, _>>();
    let mut counts = [0_usize; 256];
    for assignment in assignments {
        let rectangle = glyphs
            .get(&assignment.slot_index)
            .context("Puzzle Puyo assignment names a missing atlas slot")?
            .rectangle;
        for y in rectangle.y..rectangle.y + rectangle.height {
            for x in rectangle.x..rectangle.x + rectangle.width {
                counts[usize::from(indices[usize::try_from(y)? * 512 + usize::try_from(x)?])] += 1;
            }
        }
    }
    counts
        .iter()
        .enumerate()
        .max_by_key(|(index, count)| (**count, std::cmp::Reverse(*index)))
        .map(|(index, _)| index as u8)
        .context("could not choose Puzzle Puyo font background index")
}

pub(super) fn choose_foreground_index(
    indices: &[u8],
    palette: &[u8],
    report: &NazoFontAtlasReport,
    background: u8,
) -> Result<u8> {
    let mut used = BTreeSet::new();
    for glyph in &report.fullwidth.glyphs {
        let rectangle = glyph.rectangle;
        for y in rectangle.y..rectangle.y + rectangle.height {
            for x in rectangle.x..rectangle.x + rectangle.width {
                let index = indices[usize::try_from(y)? * 512 + usize::try_from(x)?];
                if index != background {
                    used.insert(index);
                }
            }
        }
    }
    used.into_iter()
        .max_by_key(|index| palette_score(palette, *index))
        .filter(|index| palette_score(palette, *index) > 0)
        .context("Puzzle Puyo font atlas has no visible foreground index")
}

fn palette_score(palette: &[u8], logical_index: u8) -> u32 {
    let logical = usize::from(logical_index);
    let stored = match logical & 0x18 {
        0x08 => logical + 8,
        0x10 => logical - 8,
        _ => logical,
    };
    let offset = stored * 4;
    let alpha = u32::from(palette[offset + 3].min(0x80));
    alpha
        * (u32::from(palette[offset])
            + u32::from(palette[offset + 1])
            + u32::from(palette[offset + 2]))
}

pub(super) fn render_assigned_glyphs(
    font: &Font,
    config: &RasterizationConfiguration,
    glyph_set: &FontGlyphSet,
    assignments: &[ExecutableTextGlyphAssignment],
    background: u8,
    foreground: u8,
    indices: &mut [u8],
) -> Result<()> {
    let glyphs = glyph_set
        .glyphs
        .iter()
        .map(|glyph| (glyph.index, glyph))
        .collect::<BTreeMap<_, _>>();
    for assignment in assignments {
        let character = assignment
            .character
            .chars()
            .next()
            .context("Puzzle Puyo assignment has no character")?;
        ensure!(
            assignment.character.chars().count() == 1,
            "Puzzle Puyo assignment contains multiple characters"
        );
        let rectangle = glyphs
            .get(&assignment.slot_index)
            .context("Puzzle Puyo assignment names a missing atlas slot")?
            .rectangle;
        for y in rectangle.y..rectangle.y + rectangle.height {
            for x in rectangle.x..rectangle.x + rectangle.width {
                indices[usize::try_from(y)? * 512 + usize::try_from(x)?] = background;
            }
        }
        let (metrics, coverage) = font.rasterize(character, config.pixel_size);
        ensure!(
            metrics.width <= rectangle.width as usize
                && metrics.height <= rectangle.height as usize,
            "rendered Puzzle Puyo glyph {character} does not fit its {}x{} slot",
            rectangle.width,
            rectangle.height,
        );
        let x = usize::try_from(rectangle.x)? + (rectangle.width as usize - metrics.width) / 2;
        let y = usize::try_from(rectangle.y)? + (rectangle.height as usize - metrics.height) / 2;
        let mut visible = 0_usize;
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                if coverage[row * metrics.width + column] > config.coverage_threshold {
                    indices[(y + row) * 512 + x + column] = foreground;
                    visible += 1;
                }
            }
        }
        ensure!(
            visible > 0,
            "Puzzle Puyo font rendered an empty {character}"
        );
    }
    Ok(())
}

pub(super) fn count_changed_pixels_outside_assignments(
    source: &[u8],
    replacement: &[u8],
    glyph_set: &FontGlyphSet,
    assignments: &[ExecutableTextGlyphAssignment],
) -> Result<(usize, usize)> {
    ensure!(
        source.len() == replacement.len(),
        "Puzzle Puyo font comparison lengths differ"
    );
    let assigned_slots = assignments
        .iter()
        .map(|assignment| assignment.slot_index)
        .collect::<BTreeSet<_>>();
    let mut protected_pixels = 0_usize;
    let mut changed_protected_pixels = 0_usize;
    for glyph in &glyph_set.glyphs {
        if assigned_slots.contains(&glyph.index) {
            continue;
        }
        let rectangle = glyph.rectangle;
        for y in rectangle.y..rectangle.y + rectangle.height {
            for x in rectangle.x..rectangle.x + rectangle.width {
                let offset = usize::try_from(y)? * 512 + usize::try_from(x)?;
                protected_pixels += 1;
                if source[offset] != replacement[offset] {
                    changed_protected_pixels += 1;
                }
            }
        }
    }
    Ok((protected_pixels, changed_protected_pixels))
}

#[cfg(test)]
#[path = "font_tests.rs"]
mod font_tests;
