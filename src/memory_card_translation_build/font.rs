use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use fontdue::{Font, FontSettings};
use serde::{Deserialize, Serialize};

use crate::cpx::read_symbol_bytes;
use crate::font_atlas::FontAtlasReport;
use crate::glyph_allocation::{GlyphAllocationReport, GlyphSlotPlan};
use crate::indexed_texture::{
    FileRecord, parse_file_records, parse_load_records, read_decoded_image, read_palette, sha256,
    unswizzle_psmt8_uploaded_as_psmct32,
};
use crate::memory_card_translation::MemoryCardTranslationEntry;

const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image";
const LOAD_TABLE_SYMBOL: &str = "MemLoadTexTbl";
const FONT_TEXTURE_SLOT: usize = 1;
const CUSTOM_CODE_LEAD: u8 = 0xf0;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FontConfiguration {
    pub(crate) schema: u32,
    pub(crate) id: String,
    pub(crate) expected_filename: String,
    pub(crate) sha256: String,
    pub(crate) license: String,
    pub(crate) upstream_repository: String,
    pub(crate) rasterization: RasterizationConfiguration,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RasterizationConfiguration {
    pub(crate) pixel_size: f32,
    pub(crate) coverage_threshold: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryCardGlyphAssignment {
    pub character: String,
    pub code_hex: String,
    pub slot_index: usize,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone)]
pub(crate) struct MemoryCardFontResources {
    pub(crate) image_record_index: usize,
    pub(crate) image_record: FileRecord,
    pub(crate) palette_record_index: usize,
    pub(crate) palette: Vec<u8>,
    pub(crate) linear_indices: Vec<u8>,
}

pub(crate) fn load_font(config: &FontConfiguration, path: &Path) -> Result<Font> {
    ensure!(
        config.schema == 1,
        "unsupported memory-card font config schema"
    );
    ensure!(
        path.file_name().and_then(|name| name.to_str()) == Some(&config.expected_filename),
        "memory-card font filename differs from its fixed configuration"
    );
    ensure!(
        config.rasterization.pixel_size.is_finite()
            && config.rasterization.pixel_size > 0.0
            && config.rasterization.pixel_size <= 24.0,
        "memory-card font pixel size does not fit the 24x24 cell"
    );
    ensure!(
        !config.id.is_empty()
            && !config.license.is_empty()
            && !config.upstream_repository.is_empty(),
        "memory-card font provenance is incomplete"
    );
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    ensure!(
        sha256(&bytes) == config.sha256,
        "memory-card font SHA-256 differs from its fixed configuration"
    );
    Font::from_bytes(bytes, FontSettings::default())
        .map_err(|error| anyhow::anyhow!("failed to parse memory-card font: {error}"))
}

pub(crate) fn load_font_resources(
    executable: &[u8],
    container: &[u8],
    atlas: &FontAtlasReport,
) -> Result<MemoryCardFontResources> {
    let file = object::File::parse(executable).context("failed to parse executable")?;
    let (_, load_bytes) = read_symbol_bytes(&file, LOAD_TABLE_SYMBOL)?;
    let loads = parse_load_records(&load_bytes)?;
    let image_count = loads
        .iter()
        .take_while(|record| record.operation == 1)
        .count();
    ensure!(
        image_count > FONT_TEXTURE_SLOT && loads.len() == image_count * 2,
        "memory-card font load table shape changed"
    );
    let image_load = loads[FONT_TEXTURE_SLOT];
    let palette_load = loads[image_count + FONT_TEXTURE_SLOT];
    let (_, file_list_bytes) = read_symbol_bytes(&file, IMAGE_FILE_LIST_SYMBOL)?;
    let records = parse_file_records(&file_list_bytes)?;
    let image_record = *records
        .get(image_load.file_record)
        .context("memory-card font image record is out of range")?;
    let palette_record = *records
        .get(palette_load.file_record)
        .context("memory-card font palette record is out of range")?;
    ensure!(
        image_load.file_record == atlas.atlas.image_file_record
            && palette_load.file_record == atlas.atlas.palette_file_record,
        "memory-card font resources differ from the consumer-linked atlas"
    );
    let decoded = read_decoded_image(container, image_record)?;
    ensure!(
        decoded.len()
            == usize::try_from(u64::from(image_record.width) * u64::from(image_record.height))?,
        "memory-card font CPX has unexpected trailing decoded bytes"
    );
    let linear_indices = unswizzle_psmt8_uploaded_as_psmct32(
        &decoded,
        usize::try_from(image_record.width)?,
        usize::try_from(image_record.height)?,
    )?;
    let palette = read_palette(container, palette_record, palette_load.flags & 2 != 0)?;
    Ok(MemoryCardFontResources {
        image_record_index: image_load.file_record,
        image_record,
        palette_record_index: palette_load.file_record,
        palette,
        linear_indices,
    })
}

pub(crate) fn collect_unique_hangul<'a>(
    texts: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<char>> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for text in texts {
        for character in text.chars() {
            if ('\u{ac00}'..='\u{d7a3}').contains(&character) && seen.insert(character) {
                output.push(character);
            }
        }
    }
    ensure!(
        !output.is_empty(),
        "memory-card translations contain no Hangul"
    );
    ensure!(
        output.len() <= usize::from(u8::MAX - 0x40) + 1,
        "memory-card translations exceed the custom-code range"
    );
    Ok(output)
}

pub(crate) fn select_reassignable_slots(
    allocation: &GlyphAllocationReport,
    required: usize,
) -> Result<Vec<&GlyphSlotPlan>> {
    let candidates = allocation
        .slots
        .iter()
        .filter(|slot| {
            matches!(
                slot.classification,
                "shadowed-duplicate-candidate" | "consumer-census-unused-candidate"
            )
        })
        .collect::<Vec<_>>();
    ensure!(
        candidates.len() == allocation.population.proven_reassignable_slots,
        "memory-card glyph candidate count differs from the proven allocation capacity"
    );
    ensure!(
        required <= candidates.len(),
        "memory-card translations need {required} Hangul glyphs but only {} reassignable slots are proven",
        candidates.len()
    );
    Ok(candidates.into_iter().take(required).collect())
}

pub(crate) fn choose_background_index(
    indices: &[u8],
    atlas_width: usize,
    candidates: &[&GlyphSlotPlan],
) -> Result<u8> {
    let mut counts = [0_usize; 256];
    for candidate in candidates {
        let rectangle = candidate.rectangle;
        for y in rectangle.y..rectangle.y + rectangle.height {
            for x in rectangle.x..rectangle.x + rectangle.width {
                counts[usize::from(
                    indices[usize::try_from(y)? * atlas_width + usize::try_from(x)?],
                )] += 1;
            }
        }
    }
    counts
        .iter()
        .enumerate()
        .max_by_key(|(index, count)| (**count, std::cmp::Reverse(*index)))
        .map(|(index, _)| index as u8)
        .context("could not choose memory-card font background index")
}

pub(crate) fn choose_foreground_index(
    indices: &[u8],
    palette: &[u8],
    atlas: &FontAtlasReport,
    atlas_width: usize,
    background: u8,
) -> Result<u8> {
    let mut used = BTreeSet::new();
    for glyph in &atlas.fullwidth.glyphs {
        let rectangle = glyph.rectangle;
        for y in rectangle.y..rectangle.y + rectangle.height {
            for x in rectangle.x..rectangle.x + rectangle.width {
                let index = indices[usize::try_from(y)? * atlas_width + usize::try_from(x)?];
                if index != background {
                    used.insert(index);
                }
            }
        }
    }
    used.into_iter()
        .max_by_key(|index| palette_score(palette, *index))
        .filter(|index| palette_score(palette, *index) > 0)
        .context("memory-card font atlas has no visible foreground index")
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_hangul(
    font: &Font,
    hangul: &[char],
    candidates: &[&GlyphSlotPlan],
    config: &RasterizationConfiguration,
    background: u8,
    foreground: u8,
    atlas_width: usize,
    indices: &mut [u8],
) -> Result<Vec<MemoryCardGlyphAssignment>> {
    let mut assignments = Vec::with_capacity(hangul.len());
    for (ordinal, (&character, candidate)) in hangul.iter().zip(candidates).enumerate() {
        let rectangle = candidate.rectangle;
        for y in rectangle.y..rectangle.y + rectangle.height {
            for x in rectangle.x..rectangle.x + rectangle.width {
                indices[usize::try_from(y)? * atlas_width + usize::try_from(x)?] = background;
            }
        }
        let (metrics, coverage) = font.rasterize(character, config.pixel_size);
        ensure!(
            metrics.width <= rectangle.width as usize
                && metrics.height <= rectangle.height as usize,
            "rendered memory-card glyph {character} does not fit its 24x24 slot"
        );
        let x = usize::try_from(rectangle.x)? + (rectangle.width as usize - metrics.width) / 2;
        let y = usize::try_from(rectangle.y)? + (rectangle.height as usize - metrics.height) / 2;
        let mut visible_pixels = 0_usize;
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                if coverage[row * metrics.width + column] > config.coverage_threshold {
                    indices[(y + row) * atlas_width + x + column] = foreground;
                    visible_pixels += 1;
                }
            }
        }
        ensure!(
            visible_pixels > 0,
            "memory-card font rendered an empty glyph for {character}"
        );
        let code = [
            CUSTOM_CODE_LEAD,
            0x40_u8
                .checked_add(u8::try_from(ordinal)?)
                .context("memory-card custom-code range overflow")?,
        ];
        assignments.push(MemoryCardGlyphAssignment {
            character: character.to_string(),
            code_hex: hex(&code),
            slot_index: candidate.index,
            x: rectangle.x,
            y: rectangle.y,
        });
    }
    Ok(assignments)
}

pub(crate) fn count_changed_pixels_outside_slots(
    source: &[u8],
    replacement: &[u8],
    atlas_width: usize,
    atlas_height: usize,
    editable_slots: &[&GlyphSlotPlan],
) -> Result<(usize, usize)> {
    ensure!(
        source.len() == atlas_width * atlas_height && replacement.len() == source.len(),
        "memory-card font index dimensions changed"
    );
    let mut editable = vec![false; source.len()];
    for slot in editable_slots {
        let rectangle = slot.rectangle;
        ensure!(
            usize::try_from(rectangle.x + rectangle.width)? <= atlas_width
                && usize::try_from(rectangle.y + rectangle.height)? <= atlas_height,
            "memory-card glyph slot exceeds the font atlas"
        );
        for y in rectangle.y..rectangle.y + rectangle.height {
            for x in rectangle.x..rectangle.x + rectangle.width {
                editable[usize::try_from(y)? * atlas_width + usize::try_from(x)?] = true;
            }
        }
    }
    let mut protected = 0_usize;
    let mut changed = 0_usize;
    for ((source, replacement), editable) in source.iter().zip(replacement).zip(editable) {
        if !editable {
            protected += 1;
            changed += usize::from(source != replacement);
        }
    }
    Ok((protected, changed))
}

pub(crate) fn encode_translation(
    text: &str,
    assignments: &[MemoryCardGlyphAssignment],
) -> Result<Vec<u8>> {
    let codes = assignments
        .iter()
        .map(|assignment| {
            let character = assignment
                .character
                .chars()
                .next()
                .context("memory-card assignment has no character")?;
            ensure!(
                assignment.character.chars().count() == 1,
                "memory-card assignment contains multiple characters"
            );
            let bytes = decode_hex(&assignment.code_hex)?;
            ensure!(bytes.len() == 2, "memory-card custom code is not two bytes");
            Ok((character, [bytes[0], bytes[1]]))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut output = Vec::new();
    for character in text.chars() {
        if character == '\n' {
            output.push(b'\n');
        } else if character.is_ascii() {
            output.push(character as u8);
        } else {
            output.extend_from_slice(codes.get(&character).with_context(|| {
                format!("memory-card translation has an unassigned glyph: {character}")
            })?);
        }
    }
    output.push(0);
    ensure!(
        !output[..output.len() - 1].contains(&0),
        "memory-card encoding contains an interior NUL"
    );
    Ok(output)
}

pub(crate) fn validate_protected_tokens(
    asset: &MemoryCardTranslationEntry,
    encoded: &[u8],
) -> Result<()> {
    for token in &asset.source.protected_ascii {
        let expected = asset
            .source
            .protected_ascii
            .iter()
            .filter(|candidate| *candidate == token)
            .count();
        ensure!(
            subsequence_count(encoded, token.as_bytes()) == expected,
            "protected ASCII token count changed in {}: {token}",
            asset.source.message_id
        );
    }
    for control in &asset.source.protected_runtime_controls {
        let expected = asset
            .source
            .protected_runtime_controls
            .iter()
            .filter(|candidate| *candidate == control)
            .count();
        ensure!(
            subsequence_count(encoded, control.as_bytes()) == expected,
            "protected runtime control changed in {}: {control}",
            asset.source.message_id
        );
    }
    ensure!(
        encoded.last() == Some(&0),
        "encoded message has no final NUL"
    );
    ensure!(
        !encoded[..encoded.len() - 1].contains(&0),
        "encoded message contains an interior NUL"
    );
    Ok(())
}

fn subsequence_count(source: &[u8], needle: &[u8]) -> usize {
    source
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

pub(crate) fn decode_hex(source: &str) -> Result<Vec<u8>> {
    ensure!(source.len().is_multiple_of(2), "hex string has odd length");
    (0..source.len())
        .step_by(2)
        .map(|offset| {
            u8::from_str_radix(&source[offset..offset + 2], 16)
                .with_context(|| format!("invalid hex byte at offset {offset}"))
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "font_tests.rs"]
mod font_tests;
