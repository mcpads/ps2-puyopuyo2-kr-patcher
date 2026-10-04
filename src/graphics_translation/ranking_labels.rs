use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use fontdue::{Font, FontSettings};
use serde::{Deserialize, Serialize};

use super::raster::{RgbaSurface, hashed_asset, read_rgba_surface, write_binary_mask};
use super::{HashedAsset, PixelBounds};
use crate::indexed_texture::{sha256, write_rgba_png};

const SOURCE_WIDTH: usize = 640;
const SOURCE_HEIGHT: usize = 512;
const LABEL_MASK_DILATION: u32 = 2;
const SOLO_LABEL_SEARCH: PixelBounds = PixelBounds {
    x: 0,
    y: 448,
    width: 256,
    height: 64,
};
const ENDLESS_LABEL_SEARCH: PixelBounds = PixelBounds {
    x: 256,
    y: 448,
    width: 256,
    height: 64,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RankingLabelRegionReport {
    pub schema: u32,
    pub source_png_sha256: String,
    pub width: u32,
    pub height: u32,
    pub regions: Vec<RankingLabelRegion>,
    pub protected_outside_japanese_mask: HashedAsset,
    pub partition: RankingLabelMaskPartition,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RankingLabelRegion {
    pub region_id: &'static str,
    pub source_text: &'static str,
    pub korean_text: &'static str,
    pub search_bounds: PixelBounds,
    pub bounds: PixelBounds,
    pub source_gold_seed_pixels: usize,
    pub masked_source_pixels: usize,
    pub source_pixel_mask: HashedAsset,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RankingLabelMaskPartition {
    pub japanese_region_pixels: usize,
    pub protected_pixels: usize,
    pub overlap_pixels: usize,
    pub uncovered_pixels: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RankingLabelFontConfig {
    pub schema: u32,
    pub id: String,
    pub expected_filename: String,
    pub sha256: String,
    pub license: String,
    pub reference_project: String,
    pub rasterization: RankingLabelRasterization,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RankingLabelRasterization {
    pub pixel_size: f32,
    pub coverage_threshold: u8,
    pub outline_radius: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RankingLabelDraftReport {
    pub schema: u32,
    pub source_png_sha256: String,
    pub output_png_sha256: String,
    pub font_id: String,
    pub font_sha256: String,
    pub font_license: String,
    pub reference_project: String,
    pub pixel_size_milli: u32,
    pub coverage_threshold: u8,
    pub outline_radius: u32,
    pub regions: Vec<RankingLabelDraftRegion>,
    pub changed_pixels: usize,
    pub changed_pixels_outside_label_windows: usize,
    pub output_colors_not_in_source: usize,
    pub review_status: &'static str,
    pub release_candidate_eligible: bool,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RankingLabelDraftRegion {
    pub region_id: &'static str,
    pub korean_text: &'static str,
    pub editable_bounds: PixelBounds,
    pub rendered_bounds: PixelBounds,
    pub fill_rgba: String,
    pub outline_rgba: String,
    pub erased_source_pixels: usize,
    pub rendered_fill_pixels: usize,
    pub rendered_outline_pixels: usize,
}

pub fn render_ranking_label_region_masks(
    source_png: &Path,
    solo_label_mask_path: &Path,
    endless_label_mask_path: &Path,
    protected_mask_path: &Path,
) -> Result<RankingLabelRegionReport> {
    let source = read_rgba_surface(source_png)?;
    ensure!(
        source.width == SOURCE_WIDTH && source.height == SOURCE_HEIGHT,
        "ranking label source surface is not 640x512"
    );
    let specifications = [
        (
            "solo-mode-label",
            "ひとりでぷよぷよ",
            "혼자서 뿌요뿌요",
            SOLO_LABEL_SEARCH,
            solo_label_mask_path,
        ),
        (
            "endless-mode-label",
            "とことんぷよぷよ",
            "끝없이 뿌요뿌요",
            ENDLESS_LABEL_SEARCH,
            endless_label_mask_path,
        ),
    ];

    let mut combined_source_mask = vec![false; source.width * source.height];
    let mut combined_editable_mask = vec![false; source.width * source.height];
    let mut regions = Vec::with_capacity(specifications.len());
    for (region_id, source_text, korean_text, search_bounds, output_path) in specifications {
        let gold_seed_mask = select_gold_pixels(&source, search_bounds)?;
        let source_gold_seed_pixels = gold_seed_mask.iter().filter(|selected| **selected).count();
        ensure!(
            source_gold_seed_pixels > 0,
            "ranking {region_id} has no gold source pixels"
        );
        let mask = dilate_mask(
            &gold_seed_mask,
            source.width,
            source.height,
            LABEL_MASK_DILATION,
            search_bounds,
        )?;
        let bounds = selected_bounds(&mask, source.width, source.height, region_id)?;
        ensure!(
            mask.iter()
                .zip(&combined_source_mask)
                .all(|(current, previous)| !(*current && *previous)),
            "ranking Japanese label regions overlap"
        );
        for (combined, selected) in combined_source_mask.iter_mut().zip(&mask) {
            *combined |= *selected;
        }
        for y in search_bounds.y..search_bounds.y + search_bounds.height {
            for x in search_bounds.x..search_bounds.x + search_bounds.width {
                combined_editable_mask[usize::try_from(y)? * source.width + usize::try_from(x)?] =
                    true;
            }
        }
        write_binary_mask(output_path, source.width, source.height, &mask)?;
        regions.push(RankingLabelRegion {
            region_id,
            source_text,
            korean_text,
            search_bounds,
            bounds,
            source_gold_seed_pixels,
            masked_source_pixels: mask.iter().filter(|selected| **selected).count(),
            source_pixel_mask: hashed_asset(output_path)?,
        });
    }

    let protected_mask = combined_editable_mask
        .iter()
        .map(|selected| !selected)
        .collect::<Vec<_>>();
    write_binary_mask(
        protected_mask_path,
        source.width,
        source.height,
        &protected_mask,
    )?;
    let partition = summarize_partition(&combined_editable_mask, &protected_mask)?;
    ensure!(
        partition.overlap_pixels == 0 && partition.uncovered_pixels == 0,
        "ranking label masks do not partition the source surface"
    );

    let source_bytes =
        fs::read(source_png).with_context(|| format!("failed to hash {}", source_png.display()))?;
    Ok(RankingLabelRegionReport {
        schema: 1,
        source_png_sha256: sha256(&source_bytes),
        width: u32::try_from(source.width)?,
        height: u32::try_from(source.height)?,
        regions,
        protected_outside_japanese_mask: hashed_asset(protected_mask_path)?,
        partition,
        evidence_boundary: "Each source mask follows the original gold Japanese lettering and its outline. The protected complement leaves two bounded label windows editable and protects every other source pixel, including all English. The report does not provide Korean artwork, insertion, runtime display, human approval, or release-candidate eligibility.",
    })
}

pub fn render_ranking_label_draft(
    source_png: &Path,
    font_config_path: &Path,
    font_path: &Path,
    output_png: &Path,
) -> Result<RankingLabelDraftReport> {
    let source = read_rgba_surface(source_png)?;
    ensure!(
        source.width == SOURCE_WIDTH && source.height == SOURCE_HEIGHT,
        "ranking label source surface is not 640x512"
    );
    let font_config_bytes = fs::read(font_config_path)
        .with_context(|| format!("failed to read {}", font_config_path.display()))?;
    let font_config: RankingLabelFontConfig = serde_json::from_slice(&font_config_bytes)
        .with_context(|| format!("failed to parse {}", font_config_path.display()))?;
    validate_font_config(&font_config, font_path)?;
    let font_bytes =
        fs::read(font_path).with_context(|| format!("failed to read {}", font_path.display()))?;
    ensure!(
        sha256(&font_bytes) == font_config.sha256,
        "ranking label font SHA-256 differs from the fixed configuration"
    );
    let font = Font::from_bytes(font_bytes, FontSettings::default())
        .map_err(|error| anyhow::anyhow!("failed to parse ranking label font: {error}"))?;

    let specifications = [
        ("solo-mode-label", "혼자서 뿌요뿌요", SOLO_LABEL_SEARCH),
        (
            "endless-mode-label",
            "끝없이 뿌요뿌요",
            ENDLESS_LABEL_SEARCH,
        ),
    ];
    let mut output = source.clone();
    let mut regions = Vec::with_capacity(specifications.len());
    for (region_id, korean_text, editable_bounds) in specifications {
        let gold_seeds = select_gold_pixels(&source, editable_bounds)?;
        let source_mask = dilate_mask(
            &gold_seeds,
            source.width,
            source.height,
            LABEL_MASK_DILATION,
            editable_bounds,
        )?;
        let erased_source_pixels =
            erase_source_label(&source, &mut output, &source_mask, editable_bounds)?;
        let (fill_color, outline_color) = select_source_gold_colors(&source, &gold_seeds)?;
        let (fill_mask, rendered_bounds) = render_centered_text(
            &font,
            korean_text,
            font_config.rasterization.pixel_size,
            font_config.rasterization.coverage_threshold,
            source.width,
            source.height,
            editable_bounds,
        )?;
        let outline_mask = dilate_mask(
            &fill_mask,
            source.width,
            source.height,
            font_config.rasterization.outline_radius,
            editable_bounds,
        )?;
        let rendered_fill_pixels = fill_mask.iter().filter(|selected| **selected).count();
        let rendered_outline_pixels = outline_mask
            .iter()
            .zip(&fill_mask)
            .filter(|(outline, fill)| **outline && !**fill)
            .count();
        paint_mask(&mut output, &outline_mask, outline_color)?;
        paint_mask(&mut output, &fill_mask, fill_color)?;
        regions.push(RankingLabelDraftRegion {
            region_id,
            korean_text,
            editable_bounds,
            rendered_bounds,
            fill_rgba: rgba_hex(fill_color),
            outline_rgba: rgba_hex(outline_color),
            erased_source_pixels,
            rendered_fill_pixels,
            rendered_outline_pixels,
        });
    }

    let editable_mask = windows_mask(
        source.width,
        source.height,
        &[SOLO_LABEL_SEARCH, ENDLESS_LABEL_SEARCH],
    )?;
    let (changed_pixels, changed_pixels_outside_label_windows) =
        changed_pixel_counts(&source, &output, &editable_mask)?;
    ensure!(changed_pixels > 0, "ranking label draft changed no pixels");
    ensure!(
        changed_pixels_outside_label_windows == 0,
        "ranking label draft changed protected pixels"
    );
    let output_colors_not_in_source = colors_not_in_source(&source, &output);
    ensure!(
        output_colors_not_in_source == 0,
        "ranking label draft introduced colors outside the source surface"
    );
    write_rgba_png(
        output_png,
        u32::try_from(output.width)?,
        u32::try_from(output.height)?,
        &output.pixels,
    )?;
    let source_bytes =
        fs::read(source_png).with_context(|| format!("failed to hash {}", source_png.display()))?;
    let output_bytes =
        fs::read(output_png).with_context(|| format!("failed to hash {}", output_png.display()))?;
    Ok(RankingLabelDraftReport {
        schema: 1,
        source_png_sha256: sha256(&source_bytes),
        output_png_sha256: sha256(&output_bytes),
        font_id: font_config.id,
        font_sha256: font_config.sha256,
        font_license: font_config.license,
        reference_project: font_config.reference_project,
        pixel_size_milli: (font_config.rasterization.pixel_size * 1000.0).round() as u32,
        coverage_threshold: font_config.rasterization.coverage_threshold,
        outline_radius: font_config.rasterization.outline_radius,
        regions,
        changed_pixels,
        changed_pixels_outside_label_windows,
        output_colors_not_in_source,
        review_status: "pre_human_review",
        release_candidate_eligible: false,
        evidence_boundary: "The draft reuses the exact BMJUA bytes and deterministic fill/outline direction proven by the sibling SFC ranking screen, but preserves this project's direct Korean wording. It proves bounded source-color composition only; insertion, runtime display, human approval, independent review, and release-candidate eligibility remain open.",
    })
}

fn validate_font_config(config: &RankingLabelFontConfig, font_path: &Path) -> Result<()> {
    ensure!(config.schema == 1, "unsupported ranking font configuration");
    ensure!(
        font_path.file_name().and_then(|name| name.to_str())
            == Some(config.expected_filename.as_str()),
        "ranking font filename differs from the fixed configuration"
    );
    ensure!(
        config.rasterization.pixel_size.is_finite()
            && config.rasterization.pixel_size > 0.0
            && config.rasterization.outline_radius > 0,
        "ranking font rasterization profile is invalid"
    );
    Ok(())
}

fn erase_source_label(
    source: &RgbaSurface,
    output: &mut RgbaSurface,
    source_mask: &[bool],
    limit: PixelBounds,
) -> Result<usize> {
    ensure!(
        source.width == output.width
            && source.height == output.height
            && source_mask.len() == source.width * source.height,
        "ranking label erase dimensions changed"
    );
    let mut erased = 0_usize;
    for y in limit.y..limit.y + limit.height {
        for x in limit.x..limit.x + limit.width {
            let index = usize::try_from(y)? * source.width + usize::try_from(x)?;
            if !source_mask[index] {
                continue;
            }
            let replacement = nearest_vertical_background(source, source_mask, x, y, limit)?;
            output.pixels[index * 4..index * 4 + 4].copy_from_slice(&replacement);
            erased += 1;
        }
    }
    Ok(erased)
}

fn nearest_vertical_background(
    source: &RgbaSurface,
    source_mask: &[bool],
    x: u32,
    y: u32,
    limit: PixelBounds,
) -> Result<[u8; 4]> {
    for distance in 1..limit.height {
        if let Some(candidate_y) = y.checked_sub(distance)
            && candidate_y >= limit.y
            && !source_mask[usize::try_from(candidate_y)? * source.width + usize::try_from(x)?]
        {
            return pixel(source, x, candidate_y);
        }
        let candidate_y = y + distance;
        if candidate_y < limit.y + limit.height
            && !source_mask[usize::try_from(candidate_y)? * source.width + usize::try_from(x)?]
        {
            return pixel(source, x, candidate_y);
        }
    }
    anyhow::bail!("ranking label column has no clean background pixel")
}

fn select_source_gold_colors(
    source: &RgbaSurface,
    gold_seeds: &[bool],
) -> Result<([u8; 4], [u8; 4])> {
    ensure!(
        gold_seeds.len() == source.width * source.height,
        "ranking color mask dimensions changed"
    );
    let mut counts = BTreeMap::<[u8; 4], usize>::new();
    for (index, selected) in gold_seeds.iter().enumerate() {
        if *selected {
            let color = <[u8; 4]>::try_from(&source.pixels[index * 4..index * 4 + 4])
                .expect("four-byte RGBA pixel");
            *counts.entry(color).or_default() += 1;
        }
    }
    let outline = counts
        .keys()
        .copied()
        .max_by_key(|color| gold_score(*color))
        .context("ranking label has no outline color candidate")?;
    let outline_score = gold_score(outline);
    let fill = counts
        .iter()
        .filter(|(color, _)| gold_score(**color) * 4 <= outline_score * 3)
        .max_by_key(|(_, count)| **count)
        .map(|(color, _)| *color)
        .context("ranking label has no darker fill color candidate")?;
    ensure!(
        fill != outline,
        "ranking label fill and outline colors collapsed"
    );
    Ok((fill, outline))
}

fn gold_score([red, green, blue, alpha]: [u8; 4]) -> u32 {
    u32::from(alpha) * (u32::from(red) * 2 + u32::from(green) * 2 - u32::from(blue))
}

fn render_centered_text(
    font: &Font,
    text: &str,
    pixel_size: f32,
    coverage_threshold: u8,
    surface_width: usize,
    surface_height: usize,
    bounds: PixelBounds,
) -> Result<(Vec<bool>, PixelBounds)> {
    let mut pen_x = 0.0_f32;
    let mut glyphs = Vec::new();
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for character in text.chars() {
        let (metrics, raster) = font.rasterize(character, pixel_size);
        let x = pen_x.round() as i32 + metrics.xmin;
        let y = -metrics.ymin - metrics.height as i32;
        if metrics.width > 0 && metrics.height > 0 && raster.iter().any(|value| *value != 0) {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + metrics.width as i32);
            max_y = max_y.max(y + metrics.height as i32);
        }
        pen_x += metrics.advance_width;
        glyphs.push((x, y, metrics, raster));
    }
    ensure!(
        min_x < max_x && min_y < max_y,
        "ranking text rendered no pixels"
    );
    let ink_width = u32::try_from(max_x - min_x)?;
    let ink_height = u32::try_from(max_y - min_y)?;
    ensure!(
        ink_width <= bounds.width && ink_height <= bounds.height,
        "ranking text {text:?} exceeds its editable bounds"
    );
    let target_x = bounds.x + (bounds.width - ink_width) / 2;
    let target_y = bounds.y + (bounds.height - ink_height) / 2;
    let x_shift = i32::try_from(target_x)? - min_x;
    let y_shift = i32::try_from(target_y)? - min_y;
    let mut mask = vec![false; surface_width * surface_height];
    for (glyph_x, glyph_y, metrics, raster) in glyphs {
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                if raster[row * metrics.width + column] < coverage_threshold {
                    continue;
                }
                let x = glyph_x + x_shift + i32::try_from(column)?;
                let y = glyph_y + y_shift + i32::try_from(row)?;
                ensure!(x >= 0 && y >= 0, "ranking text escaped its surface");
                let x = usize::try_from(x)?;
                let y = usize::try_from(y)?;
                ensure!(
                    x < surface_width && y < surface_height,
                    "ranking text escaped its surface"
                );
                mask[y * surface_width + x] = true;
            }
        }
    }
    Ok((
        mask,
        PixelBounds {
            x: target_x,
            y: target_y,
            width: ink_width,
            height: ink_height,
        },
    ))
}

fn paint_mask(surface: &mut RgbaSurface, mask: &[bool], color: [u8; 4]) -> Result<()> {
    ensure!(
        mask.len() == surface.width * surface.height,
        "ranking paint mask dimensions changed"
    );
    for (index, selected) in mask.iter().enumerate() {
        if *selected {
            surface.pixels[index * 4..index * 4 + 4].copy_from_slice(&color);
        }
    }
    Ok(())
}

fn windows_mask(width: usize, height: usize, windows: &[PixelBounds]) -> Result<Vec<bool>> {
    let mut mask = vec![false; width * height];
    for bounds in windows {
        ensure!(
            usize::try_from(bounds.x + bounds.width)? <= width
                && usize::try_from(bounds.y + bounds.height)? <= height,
            "ranking editable window exceeds its surface"
        );
        for y in bounds.y..bounds.y + bounds.height {
            for x in bounds.x..bounds.x + bounds.width {
                mask[usize::try_from(y)? * width + usize::try_from(x)?] = true;
            }
        }
    }
    Ok(mask)
}

fn changed_pixel_counts(
    source: &RgbaSurface,
    output: &RgbaSurface,
    editable_mask: &[bool],
) -> Result<(usize, usize)> {
    ensure!(
        source.width == output.width
            && source.height == output.height
            && editable_mask.len() == source.width * source.height,
        "ranking draft comparison dimensions changed"
    );
    let mut changed = 0_usize;
    let mut outside = 0_usize;
    for (index, (before, after)) in source
        .pixels
        .chunks_exact(4)
        .zip(output.pixels.chunks_exact(4))
        .enumerate()
    {
        if before != after {
            changed += 1;
            outside += usize::from(!editable_mask[index]);
        }
    }
    Ok((changed, outside))
}

fn colors_not_in_source(source: &RgbaSurface, output: &RgbaSurface) -> usize {
    let source_colors = source
        .pixels
        .chunks_exact(4)
        .map(|pixel| <[u8; 4]>::try_from(pixel).expect("four-byte RGBA pixel"))
        .collect::<BTreeSet<_>>();
    output
        .pixels
        .chunks_exact(4)
        .filter(|pixel| {
            let pixel = <[u8; 4]>::try_from(*pixel).expect("four-byte RGBA pixel");
            !source_colors.contains(&pixel)
        })
        .count()
}

fn select_gold_pixels(surface: &RgbaSurface, search_bounds: PixelBounds) -> Result<Vec<bool>> {
    validate_bounds(surface, search_bounds, "ranking gold label search")?;
    let mut selected = vec![false; surface.width * surface.height];
    for y in search_bounds.y..search_bounds.y + search_bounds.height {
        for x in search_bounds.x..search_bounds.x + search_bounds.width {
            let [red, green, blue, alpha] = pixel(surface, x, y)?;
            if alpha > 0
                && u16::from(red) > u16::from(blue) + 20
                && u16::from(green) > u16::from(blue) + 20
            {
                selected[usize::try_from(y)? * surface.width + usize::try_from(x)?] = true;
            }
        }
    }
    Ok(selected)
}

fn dilate_mask(
    source: &[bool],
    width: usize,
    height: usize,
    radius: u32,
    limit: PixelBounds,
) -> Result<Vec<bool>> {
    ensure!(
        source.len() == width * height,
        "source mask dimensions changed"
    );
    ensure!(
        usize::try_from(limit.x + limit.width)? <= width
            && usize::try_from(limit.y + limit.height)? <= height,
        "mask dilation limit exceeds its surface"
    );
    let mut dilated = vec![false; source.len()];
    for y in limit.y..limit.y + limit.height {
        for x in limit.x..limit.x + limit.width {
            if !source[usize::try_from(y)? * width + usize::try_from(x)?] {
                continue;
            }
            let left = x.saturating_sub(radius).max(limit.x);
            let top = y.saturating_sub(radius).max(limit.y);
            let right = (x + radius + 1).min(limit.x + limit.width);
            let bottom = (y + radius + 1).min(limit.y + limit.height);
            for selected_y in top..bottom {
                for selected_x in left..right {
                    dilated[usize::try_from(selected_y)? * width + usize::try_from(selected_x)?] =
                        true;
                }
            }
        }
    }
    Ok(dilated)
}

fn selected_bounds(mask: &[bool], width: usize, height: usize, label: &str) -> Result<PixelBounds> {
    ensure!(
        mask.len() == width * height,
        "selected mask dimensions changed"
    );
    let mut left = width;
    let mut top = height;
    let mut right = 0_usize;
    let mut bottom = 0_usize;
    for y in 0..height {
        for x in 0..width {
            if !mask[y * width + x] {
                continue;
            }
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    ensure!(left < right && top < bottom, "{label} mask is empty");
    Ok(PixelBounds {
        x: u32::try_from(left)?,
        y: u32::try_from(top)?,
        width: u32::try_from(right - left)?,
        height: u32::try_from(bottom - top)?,
    })
}

fn summarize_partition(
    japanese_mask: &[bool],
    protected_mask: &[bool],
) -> Result<RankingLabelMaskPartition> {
    ensure!(
        japanese_mask.len() == protected_mask.len(),
        "ranking label partition dimensions changed"
    );
    let mut report = RankingLabelMaskPartition {
        japanese_region_pixels: 0,
        protected_pixels: 0,
        overlap_pixels: 0,
        uncovered_pixels: 0,
    };
    for (japanese, protected) in japanese_mask.iter().zip(protected_mask) {
        report.japanese_region_pixels += usize::from(*japanese);
        report.protected_pixels += usize::from(*protected);
        report.overlap_pixels += usize::from(*japanese && *protected);
        report.uncovered_pixels += usize::from(!*japanese && !*protected);
    }
    Ok(report)
}

fn validate_bounds(surface: &RgbaSurface, bounds: PixelBounds, label: &str) -> Result<()> {
    ensure!(
        bounds.width > 0
            && bounds.height > 0
            && usize::try_from(bounds.x + bounds.width)? <= surface.width
            && usize::try_from(bounds.y + bounds.height)? <= surface.height,
        "{label} bounds exceed the source surface"
    );
    Ok(())
}

fn pixel(surface: &RgbaSurface, x: u32, y: u32) -> Result<[u8; 4]> {
    let offset = (usize::try_from(y)? * surface.width + usize::try_from(x)?) * 4;
    Ok(<[u8; 4]>::try_from(&surface.pixels[offset..offset + 4]).expect("four-byte RGBA pixel"))
}

fn rgba_hex(color: [u8; 4]) -> String {
    color.iter().map(|value| format!("{value:02x}")).collect()
}

#[cfg(test)]
#[path = "ranking_labels_tests.rs"]
mod ranking_labels_tests;
