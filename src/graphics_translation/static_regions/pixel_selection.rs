use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use super::super::raster::{RgbaSurface, selected_bounds};
use super::PixelBounds;
use crate::indexed_texture::sha256;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelRange {
    pub minimum: u8,
    pub maximum: u8,
}

impl ChannelRange {
    fn contains(self, value: u8) -> bool {
        self.minimum <= value && value <= self.maximum
    }
}

impl RgbaColor {
    fn from_pixel(pixel: &[u8]) -> Self {
        Self {
            red: pixel[0],
            green: pixel[1],
            blue: pixel[2],
            alpha: pixel[3],
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegionPixelSelection {
    #[default]
    Visible,
    ExactRgbaColors {
        colors: Vec<RgbaColor>,
    },
    NearGrayscale {
        maximum_rgb_delta: u8,
    },
    RgbaRanges {
        red: ChannelRange,
        green: ChannelRange,
        blue: ChannelRange,
        alpha: ChannelRange,
    },
    AnyOf {
        selections: Vec<RegionPixelSelection>,
    },
    PaletteIndices {
        indices: Vec<u8>,
    },
    ReviewedPixelMask {
        path: String,
        sha256: String,
    },
}

pub(super) fn select_region_pixels(
    surface: &RgbaSurface,
    linear_indices: &[u8],
    bounds: PixelBounds,
    protected_subregions: &[PixelBounds],
    selection: &RegionPixelSelection,
    claimed_pixels: &mut [bool],
    context: &str,
) -> Result<Vec<bool>> {
    let x = usize::try_from(bounds.x)?;
    let y = usize::try_from(bounds.y)?;
    let width = usize::try_from(bounds.width)?;
    let height = usize::try_from(bounds.height)?;
    ensure!(
        width > 0
            && height > 0
            && x + width <= surface.width
            && y + height <= surface.height
            && claimed_pixels.len() == surface.width * surface.height
            && linear_indices.len() == surface.width * surface.height,
        "declared static region falls outside its surface: {context}"
    );
    validate_protected_subregions(bounds, protected_subregions, context)?;

    let declared_colors = validate_selection(selection, true, context)?;
    let declared_indices = declared_palette_indices(selection, context)?;
    let reviewed_mask = load_reviewed_pixel_mask(selection, surface, bounds, context)?;
    let mut observed_colors = BTreeSet::new();
    let mut observed_indices = BTreeSet::new();
    let mut selected = vec![false; surface.width * surface.height];
    for row in y..y + height {
        for column in x..x + width {
            if protected_subregions
                .iter()
                .any(|protected| contains(*protected, column, row))
            {
                continue;
            }
            let index = row * surface.width + column;
            let pixel = &surface.pixels[index * 4..index * 4 + 4];
            let color = RgbaColor::from_pixel(pixel);
            let palette_index = linear_indices[index];
            let matches = reviewed_mask.as_ref().map_or_else(
                || selection.matches(color, palette_index),
                |mask| mask[index],
            );
            if !matches {
                continue;
            }
            ensure!(
                !claimed_pixels[index],
                "declared static regions overlap at pixel ({column}, {row}): {context}"
            );
            selected[index] = true;
            claimed_pixels[index] = true;
            observed_colors.insert(color);
            observed_indices.insert(palette_index);
        }
    }
    if !declared_colors.is_empty() {
        ensure!(
            declared_colors.is_subset(&observed_colors),
            "exact RGBA selection contains a color absent from its declared bounds: {context}"
        );
    }
    if !declared_indices.is_empty() {
        ensure!(
            declared_indices.is_subset(&observed_indices),
            "palette-index selection contains an index absent from its declared bounds: {context}"
        );
    }
    let actual_bounds = selected_bounds(&selected, surface.width, surface.height, context)?;
    ensure!(
        actual_bounds == bounds
            || selected_bounds_changed_only_at_protected_edges(
                bounds,
                actual_bounds,
                protected_subregions,
            ),
        "declared static region is not tight around its selected pixels: {context}; declared {bounds:?}, actual {actual_bounds:?}"
    );
    Ok(selected)
}

fn selected_bounds_changed_only_at_protected_edges(
    declared: PixelBounds,
    selected: PixelBounds,
    protected_subregions: &[PixelBounds],
) -> bool {
    if protected_subregions.is_empty() {
        return false;
    }
    let declared_right = declared.x + declared.width;
    let declared_bottom = declared.y + declared.height;
    let selected_right = selected.x + selected.width;
    let selected_bottom = selected.y + selected.height;
    if selected.x < declared.x
        || selected.y < declared.y
        || selected_right > declared_right
        || selected_bottom > declared_bottom
    {
        return false;
    }
    let protected = |x: u32, y: u32| {
        protected_subregions
            .iter()
            .any(|bounds| contains(*bounds, x as usize, y as usize))
    };
    (selected.x == declared.x || (declared.y..declared_bottom).all(|y| protected(declared.x, y)))
        && (selected_right == declared_right
            || (declared.y..declared_bottom).all(|y| protected(declared_right - 1, y)))
        && (selected.y == declared.y
            || (declared.x..declared_right).all(|x| protected(x, declared.y)))
        && (selected_bottom == declared_bottom
            || (declared.x..declared_right).all(|x| protected(x, declared_bottom - 1)))
}

fn validate_protected_subregions(
    bounds: PixelBounds,
    protected_subregions: &[PixelBounds],
    context: &str,
) -> Result<()> {
    let outer_x = usize::try_from(bounds.x)?;
    let outer_y = usize::try_from(bounds.y)?;
    let outer_width = usize::try_from(bounds.width)?;
    let outer_height = usize::try_from(bounds.height)?;
    for (index, protected) in protected_subregions.iter().enumerate() {
        let x = usize::try_from(protected.x)?;
        let y = usize::try_from(protected.y)?;
        let width = usize::try_from(protected.width)?;
        let height = usize::try_from(protected.height)?;
        ensure!(
            width > 0
                && height > 0
                && x >= outer_x
                && y >= outer_y
                && x + width <= outer_x + outer_width
                && y + height <= outer_y + outer_height,
            "protected static subregion falls outside its editable bounds: {context}"
        );
        ensure!(
            protected_subregions[..index]
                .iter()
                .all(|other| !rectangles_overlap(*other, *protected)),
            "protected static subregions overlap: {context}"
        );
    }
    Ok(())
}

pub(super) fn contains(bounds: PixelBounds, x: usize, y: usize) -> bool {
    let left = bounds.x as usize;
    let top = bounds.y as usize;
    left <= x && x < left + bounds.width as usize && top <= y && y < top + bounds.height as usize
}

pub(super) fn rectangles_overlap(left: PixelBounds, right: PixelBounds) -> bool {
    let left_x = u64::from(left.x);
    let left_y = u64::from(left.y);
    let right_x = u64::from(right.x);
    let right_y = u64::from(right.y);
    left_x < right_x + u64::from(right.width)
        && right_x < left_x + u64::from(left.width)
        && left_y < right_y + u64::from(right.height)
        && right_y < left_y + u64::from(left.height)
}

impl RegionPixelSelection {
    pub(super) fn is_source_pixel_stable(&self) -> bool {
        match self {
            Self::PaletteIndices { .. } | Self::ReviewedPixelMask { .. } => true,
            Self::AnyOf { selections } => selections
                .iter()
                .all(RegionPixelSelection::is_source_pixel_stable),
            _ => false,
        }
    }

    fn matches(&self, color: RgbaColor, palette_index: u8) -> bool {
        match self {
            Self::Visible => color.alpha != 0,
            Self::ExactRgbaColors { colors } => colors.contains(&color),
            Self::NearGrayscale { maximum_rgb_delta } => {
                let minimum = color.red.min(color.green).min(color.blue);
                let maximum = color.red.max(color.green).max(color.blue);
                color.alpha != 0 && maximum - minimum <= *maximum_rgb_delta
            }
            Self::RgbaRanges {
                red,
                green,
                blue,
                alpha,
            } => {
                red.contains(color.red)
                    && green.contains(color.green)
                    && blue.contains(color.blue)
                    && alpha.contains(color.alpha)
            }
            Self::AnyOf { selections } => selections
                .iter()
                .any(|selection| selection.matches(color, palette_index)),
            Self::PaletteIndices { indices } => indices.contains(&palette_index),
            Self::ReviewedPixelMask { .. } => false,
        }
    }
}

fn validate_selection(
    selection: &RegionPixelSelection,
    top_level: bool,
    context: &str,
) -> Result<BTreeSet<RgbaColor>> {
    match selection {
        RegionPixelSelection::Visible => {
            ensure!(
                top_level,
                "visible selection makes a composite color selection meaningless: {context}"
            );
            Ok(BTreeSet::new())
        }
        RegionPixelSelection::ExactRgbaColors { colors } => {
            let unique = colors.iter().copied().collect::<BTreeSet<_>>();
            ensure!(
                !colors.is_empty()
                    && unique.len() == colors.len()
                    && colors.iter().all(|color| color.alpha != 0),
                "exact RGBA selection must declare unique visible colors: {context}"
            );
            Ok(unique)
        }
        RegionPixelSelection::NearGrayscale { .. } => Ok(BTreeSet::new()),
        RegionPixelSelection::RgbaRanges {
            red,
            green,
            blue,
            alpha,
        } => {
            ensure!(
                red.minimum <= red.maximum
                    && green.minimum <= green.maximum
                    && blue.minimum <= blue.maximum
                    && alpha.minimum > 0
                    && alpha.minimum <= alpha.maximum,
                "RGBA selection ranges are empty or admit transparent pixels: {context}"
            );
            Ok(BTreeSet::new())
        }
        RegionPixelSelection::AnyOf { selections } => {
            ensure!(
                !selections.is_empty(),
                "composite color selection has no alternatives: {context}"
            );
            let mut colors = BTreeSet::new();
            for child in selections {
                colors.extend(validate_selection(child, false, context)?);
            }
            Ok(colors)
        }
        RegionPixelSelection::PaletteIndices { indices } => {
            let unique = indices.iter().copied().collect::<BTreeSet<_>>();
            ensure!(
                !indices.is_empty() && unique.len() == indices.len(),
                "palette-index selection must declare unique indices: {context}"
            );
            Ok(BTreeSet::new())
        }
        RegionPixelSelection::ReviewedPixelMask { path, sha256 } => {
            ensure!(
                top_level && is_safe_relative_path(path) && is_sha256(sha256),
                "reviewed pixel mask must declare a safe relative path and lowercase SHA-256: {context}"
            );
            Ok(BTreeSet::new())
        }
    }
}

fn declared_palette_indices(
    selection: &RegionPixelSelection,
    context: &str,
) -> Result<BTreeSet<u8>> {
    match selection {
        RegionPixelSelection::PaletteIndices { indices } => {
            let unique = indices.iter().copied().collect::<BTreeSet<_>>();
            ensure!(
                !indices.is_empty() && unique.len() == indices.len(),
                "palette-index selection must declare unique indices: {context}"
            );
            Ok(unique)
        }
        RegionPixelSelection::AnyOf { selections } => {
            let mut indices = BTreeSet::new();
            for child in selections {
                indices.extend(declared_palette_indices(child, context)?);
            }
            Ok(indices)
        }
        RegionPixelSelection::ReviewedPixelMask { .. } => Ok(BTreeSet::new()),
        _ => Ok(BTreeSet::new()),
    }
}

fn load_reviewed_pixel_mask(
    selection: &RegionPixelSelection,
    surface: &RgbaSurface,
    bounds: PixelBounds,
    context: &str,
) -> Result<Option<Vec<bool>>> {
    let RegionPixelSelection::ReviewedPixelMask { path, sha256: hash } = selection else {
        return Ok(None);
    };
    let bytes = fs::read(path)?;
    ensure!(
        sha256(&bytes) == *hash,
        "reviewed pixel mask SHA-256 differs: {context}"
    );
    let mask_surface = super::super::raster::read_rgba_surface(Path::new(path))?;
    let full_surface = mask_surface.width == surface.width && mask_surface.height == surface.height;
    let region_crop = mask_surface.width == bounds.width as usize
        && mask_surface.height == bounds.height as usize;
    ensure!(
        full_surface || region_crop,
        "reviewed pixel mask dimensions differ from its source surface: {context}"
    );
    let mut mask = vec![false; surface.width * surface.height];
    for (index, pixel) in mask_surface.pixels.chunks_exact(4).enumerate() {
        let selected = match pixel {
            [0, 0, 0, 255] => false,
            [255, 255, 255, 255] => true,
            _ => anyhow::bail!("reviewed pixel mask is not binary at pixel {index}: {context}"),
        };
        if selected {
            let x = index % mask_surface.width + if full_surface { 0 } else { bounds.x as usize };
            let y = index / mask_surface.width + if full_surface { 0 } else { bounds.y as usize };
            ensure!(
                contains(bounds, x, y),
                "reviewed pixel mask selects outside its declared bounds: {context}"
            );
            ensure!(
                x < surface.width && y < surface.height,
                "reviewed pixel mask exceeds its source surface: {context}"
            );
            mask[y * surface.width + x] = true;
        }
    }
    Ok(Some(mask))
}

fn is_safe_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
#[path = "pixel_selection_tests.rs"]
mod pixel_selection_tests;
