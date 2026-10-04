use std::cmp::Reverse;
use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::bitmap_font::GlyphPixel;
use crate::indexed_texture::colorize_psmt8;

pub(crate) const OVERLAY_TRANSPARENT: [u8; 4] = [0, 0, 0, 0];
pub(crate) const OVERLAY_OUTLINE: [u8; 4] = [0, 0, 0, u8::MAX];
pub(crate) const OVERLAY_FILL: [u8; 4] = [u8::MAX, u8::MAX, u8::MAX, u8::MAX];

pub(crate) fn select_preview_colors(
    source_rgba: &[u8],
    source_mask: &[bool],
) -> Result<([u8; 4], [u8; 4])> {
    select_preview_colors_with_fill(source_rgba, source_mask, None)
}

pub(super) fn select_preview_colors_with_fill(
    source_rgba: &[u8],
    source_mask: &[bool],
    source_fill_color: Option<[u8; 4]>,
) -> Result<([u8; 4], [u8; 4])> {
    ensure!(
        source_rgba.len() == source_mask.len() * 4,
        "functional UI source color dimensions changed"
    );
    let mut colors = BTreeMap::<[u8; 4], usize>::new();
    for (pixel, selected) in source_mask.iter().enumerate() {
        if !selected {
            continue;
        }
        let rgba: [u8; 4] = source_rgba[pixel * 4..pixel * 4 + 4]
            .try_into()
            .expect("RGBA pixel has four bytes");
        if rgba[3] != 0 {
            *colors.entry(rgba).or_default() += 1;
        }
    }
    ensure!(
        !colors.is_empty(),
        "functional UI source mask has no visible color"
    );
    let outline = colors
        .keys()
        .min_by_key(|rgba| (brightness(**rgba), **rgba))
        .copied()
        .expect("non-empty color set");
    if let Some(fill) = source_fill_color {
        ensure!(
            colors.contains_key(&fill),
            "declared text fill color is absent from the visible source mask"
        );
        ensure!(
            fill != outline,
            "declared text fill color equals the outline"
        );
        return Ok((outline, fill));
    }
    let fill = colors
        .iter()
        .filter(|(rgba, _)| colors.len() == 1 || **rgba != outline)
        .max_by_key(|(rgba, usage)| (**usage, brightness(**rgba), **rgba))
        .map(|(rgba, _)| *rgba)
        .expect("non-empty color set");
    Ok((outline, fill))
}

pub(super) fn select_source_role_indices(
    source_indices: &[u8],
    palette: &[u8],
    source_mask: &[bool],
    source_fill_color: Option<[u8; 4]>,
) -> Result<(u8, u8)> {
    ensure!(
        source_indices.len() == source_mask.len() && palette.len() == 256 * 4,
        "static UI role-index inputs changed"
    );
    let mut usage = BTreeMap::<u8, usize>::new();
    for (index, selected) in source_indices.iter().zip(source_mask) {
        if *selected {
            *usage.entry(*index).or_default() += 1;
        }
    }
    ensure!(
        !usage.is_empty(),
        "static UI source mask has no palette indices"
    );
    let colors = usage
        .keys()
        .map(|index| {
            let rgba: [u8; 4] = colorize_psmt8(&[*index], palette)
                .try_into()
                .expect("one palette index produces one RGBA pixel");
            (*index, rgba)
        })
        .collect::<BTreeMap<_, _>>();
    let visible = usage
        .keys()
        .copied()
        .filter(|index| colors[index][3] != 0)
        .collect::<Vec<_>>();
    ensure!(
        !visible.is_empty(),
        "static UI source mask has no visible palette index"
    );
    let outline = visible
        .iter()
        .copied()
        .min_by_key(|index| {
            (
                brightness(colors[index]),
                std::cmp::Reverse(usage[index]),
                *index,
            )
        })
        .expect("visible source role indices are non-empty");
    if let Some(fill_color) = source_fill_color {
        let fill = visible
            .iter()
            .copied()
            .filter(|index| colors[index] == fill_color)
            .max_by_key(|index| (usage[index], std::cmp::Reverse(*index)))
            .context("declared text fill color is absent from the visible source mask")?;
        ensure!(
            fill_color != colors[&outline],
            "declared text fill color equals the outline"
        );
        return Ok((outline, fill));
    }
    let fill = visible
        .iter()
        .copied()
        .filter(|index| visible.len() == 1 || *index != outline)
        .max_by_key(|index| {
            (
                usage[index],
                brightness(colors[index]),
                std::cmp::Reverse(*index),
            )
        })
        .expect("visible source role indices are non-empty");
    Ok((outline, fill))
}

fn overlay_role(rgba: &[u8]) -> Result<Option<GlyphPixel>> {
    ensure!(rgba.len() == 4, "static UI overlay pixel is not RGBA");
    match rgba {
        [0, 0, 0, 0] => Ok(None),
        [0, 0, 0, 255] => Ok(Some(GlyphPixel::Outline)),
        [255, 255, 255, 255] => Ok(Some(GlyphPixel::Fill)),
        _ => anyhow::bail!("static UI overlay contains a non-role pixel"),
    }
}

pub(super) enum RegionOverlayPalette {
    GlyphRoles { outline_index: u8, fill_index: u8 },
    ExactSourceColors(BTreeMap<[u8; 4], u8>),
}

impl RegionOverlayPalette {
    pub(super) fn palette_index(&self, rgba: &[u8]) -> Result<Option<u8>> {
        ensure!(rgba.len() == 4, "static graphics overlay pixel is not RGBA");
        if rgba == OVERLAY_TRANSPARENT {
            return Ok(None);
        }
        match self {
            Self::GlyphRoles {
                outline_index,
                fill_index,
            } => Ok(match overlay_role(rgba)? {
                None => None,
                Some(GlyphPixel::Transparent) => unreachable!(),
                Some(GlyphPixel::Outline) => Some(*outline_index),
                Some(GlyphPixel::Fill) => Some(*fill_index),
            }),
            Self::ExactSourceColors(colors) => {
                let color: [u8; 4] = rgba
                    .try_into()
                    .expect("an RGBA pixel always has four bytes");
                ensure!(
                    color[3] == u8::MAX,
                    "stylized artwork pixel has partial alpha"
                );
                Ok(Some(*colors.get(&color).with_context(|| {
                    format!(
                        "stylized artwork color [{}, {}, {}, {}] is absent or ambiguous in its source Japanese mask",
                        color[0], color[1], color[2], color[3]
                    )
                })?))
            }
        }
    }
}

pub(super) fn select_source_color_indices(
    source_indices: &[u8],
    palette: &[u8],
    source_mask: &[bool],
) -> Result<BTreeMap<[u8; 4], u8>> {
    ensure!(
        source_indices.len() == source_mask.len() && palette.len() == 256 * 4,
        "stylized source-color inputs changed"
    );
    let used_indices = source_indices
        .iter()
        .zip(source_mask)
        .filter_map(|(index, selected)| selected.then_some(*index))
        .collect::<std::collections::BTreeSet<_>>();
    let mut colors = BTreeMap::new();
    for index in used_indices {
        let color: [u8; 4] = colorize_psmt8(&[index], palette)
            .try_into()
            .expect("one palette index produces one RGBA pixel");
        if color[3] == 0 {
            continue;
        }
        ensure!(
            colors.insert(color, index).is_none(),
            "stylized source Japanese mask uses more than one palette index for the same visible RGBA color"
        );
    }
    ensure!(
        !colors.is_empty(),
        "stylized source Japanese mask has no visible palette colors"
    );
    Ok(colors)
}

pub(crate) fn select_composition_color_indices(
    source_indices: &[u8],
    palette: &[u8],
    width: usize,
    height: usize,
    bounds: super::super::PixelBounds,
    maximum_colors: Option<usize>,
) -> Result<BTreeMap<[u8; 4], u8>> {
    ensure!(
        source_indices.len() == width * height
            && palette.len() == 256 * 4
            && bounds.width > 0
            && bounds.height > 0
            && usize::try_from(bounds.x + bounds.width)? <= width
            && usize::try_from(bounds.y + bounds.height)? <= height,
        "stylized composition palette bounds or dimensions changed"
    );
    let left = usize::try_from(bounds.x)?;
    let top = usize::try_from(bounds.y)?;
    let right = usize::try_from(bounds.x + bounds.width)?;
    let bottom = usize::try_from(bounds.y + bounds.height)?;
    let mut usage = BTreeMap::<[u8; 4], BTreeMap<u8, usize>>::new();
    for y in top..bottom {
        for x in left..right {
            let index = source_indices[y * width + x];
            let rgba: [u8; 4] = colorize_psmt8(&[index], palette)
                .try_into()
                .expect("one palette index produces one RGBA pixel");
            if rgba[3] == 0 {
                continue;
            }
            *usage.entry(rgba).or_default().entry(index).or_default() += 1;
        }
    }
    ensure!(
        !usage.is_empty(),
        "stylized artwork composition has no visible palette colors"
    );
    ensure!(
        maximum_colors.is_none_or(|maximum| (2..=256).contains(&maximum)),
        "stylized artwork composition palette limit is outside 2..=256"
    );
    let mut colors = usage
        .into_iter()
        .map(|(rgba, indices)| {
            let total_pixels = indices.values().sum::<usize>();
            let index = indices
                .into_iter()
                .max_by_key(|(index, pixels)| (*pixels, Reverse(*index)))
                .expect("a visible composition color has a palette index")
                .0;
            (rgba, index, total_pixels)
        })
        .collect::<Vec<_>>();
    if let Some(maximum) = maximum_colors {
        colors.sort_by_key(|(rgba, index, pixels)| (Reverse(*pixels), *rgba, *index));
        colors.truncate(maximum);
    }
    Ok(colors
        .into_iter()
        .map(|(rgba, index, _)| (rgba, index))
        .collect())
}

fn brightness(rgba: [u8; 4]) -> u16 {
    u16::from(rgba[0]) + u16::from(rgba[1]) + u16::from(rgba[2])
}
