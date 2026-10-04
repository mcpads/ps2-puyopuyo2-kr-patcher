use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::PixelBounds;
use super::raster::{RgbaSurface, read_rgba_surface};

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteIndexCensus {
    pub schema: u32,
    pub source_surface_id: String,
    pub preview_png_path: String,
    pub preview_png_sha256: String,
    pub linear_indices_sha256: String,
    pub bounds: PixelBounds,
    pub pixels_in_bounds: usize,
    pub indices: Vec<PaletteIndexOccurrence>,
    pub evidence_boundary: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteIndexOccurrence {
    pub palette_index: u8,
    pub pixel_count: usize,
    pub occurrence_bounds: PixelBounds,
    pub colors: Vec<RgbaOccurrence>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RgbaOccurrence {
    pub color: RgbaColor,
    pub pixel_count: usize,
}

pub struct PaletteIndexCensusInputs<'a> {
    pub source_surface_id: &'a str,
    pub preview_png_path: &'a Path,
    pub preview_png_sha256: &'a str,
    pub linear_indices_sha256: &'a str,
    pub linear_indices: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub bounds: PixelBounds,
}

pub fn survey_palette_indices(inputs: PaletteIndexCensusInputs<'_>) -> Result<PaletteIndexCensus> {
    let preview_bytes = fs::read(inputs.preview_png_path)
        .with_context(|| format!("failed to hash {}", inputs.preview_png_path.display()))?;
    ensure!(
        sha256(&preview_bytes) == inputs.preview_png_sha256,
        "palette-index census preview SHA-256 differs"
    );
    ensure!(
        sha256(inputs.linear_indices) == inputs.linear_indices_sha256,
        "palette-index census linear indices SHA-256 differs"
    );
    let surface = read_rgba_surface(inputs.preview_png_path)?;
    ensure!(
        surface.width == usize::try_from(inputs.width)?
            && surface.height == usize::try_from(inputs.height)?,
        "palette-index census preview dimensions differ"
    );
    let indices = census_palette_indices(&surface, inputs.linear_indices, inputs.bounds)?;
    Ok(PaletteIndexCensus {
        schema: 1,
        source_surface_id: inputs.source_surface_id.to_string(),
        preview_png_path: inputs.preview_png_path.to_string_lossy().into_owned(),
        preview_png_sha256: inputs.preview_png_sha256.to_string(),
        linear_indices_sha256: inputs.linear_indices_sha256.to_string(),
        bounds: inputs.bounds,
        pixels_in_bounds: usize::try_from(inputs.bounds.width)?
            * usize::try_from(inputs.bounds.height)?,
        indices,
        evidence_boundary: "This exact-source rectangle census reports palette indices, rendered RGBA colors, occurrence counts, and occurrence bounds. It does not classify text pixels, approve an editable region, prove a replacement, or establish runtime consumption or release readiness.".to_string(),
    })
}

fn census_palette_indices(
    surface: &RgbaSurface,
    linear_indices: &[u8],
    bounds: PixelBounds,
) -> Result<Vec<PaletteIndexOccurrence>> {
    let left = usize::try_from(bounds.x)?;
    let top = usize::try_from(bounds.y)?;
    let width = usize::try_from(bounds.width)?;
    let height = usize::try_from(bounds.height)?;
    ensure!(
        width > 0
            && height > 0
            && left + width <= surface.width
            && top + height <= surface.height
            && linear_indices.len() == surface.width * surface.height,
        "palette-index census bounds or dimensions are invalid"
    );

    let mut occurrences = BTreeMap::<u8, IndexAccumulator>::new();
    for y in top..top + height {
        for x in left..left + width {
            let pixel_offset = y * surface.width + x;
            let rgba_offset = pixel_offset * 4;
            let color = RgbaColor {
                red: surface.pixels[rgba_offset],
                green: surface.pixels[rgba_offset + 1],
                blue: surface.pixels[rgba_offset + 2],
                alpha: surface.pixels[rgba_offset + 3],
            };
            occurrences
                .entry(linear_indices[pixel_offset])
                .or_insert_with(|| IndexAccumulator::new(x, y))
                .record(x, y, color);
        }
    }
    occurrences
        .into_iter()
        .map(|(palette_index, occurrence)| occurrence.finish(palette_index))
        .collect()
}

struct IndexAccumulator {
    pixel_count: usize,
    minimum_x: usize,
    minimum_y: usize,
    maximum_x: usize,
    maximum_y: usize,
    colors: BTreeMap<RgbaColor, usize>,
}

impl IndexAccumulator {
    fn new(x: usize, y: usize) -> Self {
        Self {
            pixel_count: 0,
            minimum_x: x,
            minimum_y: y,
            maximum_x: x,
            maximum_y: y,
            colors: BTreeMap::new(),
        }
    }

    fn record(&mut self, x: usize, y: usize, color: RgbaColor) {
        self.pixel_count += 1;
        self.minimum_x = self.minimum_x.min(x);
        self.minimum_y = self.minimum_y.min(y);
        self.maximum_x = self.maximum_x.max(x);
        self.maximum_y = self.maximum_y.max(y);
        *self.colors.entry(color).or_default() += 1;
    }

    fn finish(self, palette_index: u8) -> Result<PaletteIndexOccurrence> {
        Ok(PaletteIndexOccurrence {
            palette_index,
            pixel_count: self.pixel_count,
            occurrence_bounds: PixelBounds {
                x: u32::try_from(self.minimum_x)?,
                y: u32::try_from(self.minimum_y)?,
                width: u32::try_from(self.maximum_x - self.minimum_x + 1)?,
                height: u32::try_from(self.maximum_y - self.minimum_y + 1)?,
            },
            colors: self
                .colors
                .into_iter()
                .map(|(color, pixel_count)| RgbaOccurrence { color, pixel_count })
                .collect(),
        })
    }
}

fn sha256(source: &[u8]) -> String {
    format!("{:x}", Sha256::digest(source))
}

#[cfg(test)]
#[path = "palette_index_census_tests.rs"]
mod palette_index_census_tests;
