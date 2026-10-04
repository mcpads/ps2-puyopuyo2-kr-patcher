use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::raster::{
    RgbaSurface as RgbaImage, hashed_asset as hashed_output_asset,
    read_rgba_surface as read_rgba_png, verify_hashed_asset, write_binary_mask as write_mask_png,
};
use super::{HashedAsset, PixelBounds};
use crate::indexed_texture::{sha256, write_rgba_png};

const EDITABLE_BOUNDS: PixelBounds = PixelBounds {
    x: 220,
    y: 0,
    width: 420,
    height: 160,
};
const LETTERING_PLACEMENT: PixelBounds = PixelBounds {
    x: 244,
    y: 0,
    width: 360,
    height: 92,
};
const DRAFT_QUANTIZATION_COLOR_LIMIT: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TitleDraftAsset {
    pub schema: u32,
    pub candidate_id: String,
    pub expected_text: String,
    pub use_case: String,
    pub tool_path: String,
    pub generation_prompt: String,
    pub generated_source: HashedAsset,
    pub transparent_master: HashedAsset,
    pub japanese_source_components: Vec<String>,
    pub protected_nontext_components: Vec<String>,
    pub protected_english_components: Vec<String>,
    pub japanese_source_mask: HashedAsset,
    pub protected_nontext_mask: HashedAsset,
    pub protected_english_mask: HashedAsset,
    pub review_status: String,
    pub evidence_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TitleTranslationDraftReport {
    pub schema: u32,
    pub candidate_id: String,
    pub expected_text: String,
    pub source_png_sha256: String,
    pub transparent_master_sha256: String,
    pub output_png_sha256: String,
    pub width: u32,
    pub height: u32,
    pub editable_bounds: PixelBounds,
    pub lettering_placement: PixelBounds,
    pub master_alpha_bounds: PixelBounds,
    pub source_visible_components: Vec<VisibleComponent>,
    pub japanese_source_mask: HashedAsset,
    pub protected_nontext_mask: HashedAsset,
    pub protected_english_mask: HashedAsset,
    pub changed_pixels: usize,
    pub changed_pixels_outside_editable_bounds: usize,
    pub changed_protected_nontext_pixels: usize,
    pub changed_protected_english_pixels: usize,
    pub source_color_count: usize,
    pub japanese_source_color_count: usize,
    pub draft_quantization_color_limit: usize,
    pub changed_output_color_count: usize,
    pub output_colors_not_in_source: usize,
    pub review_status: &'static str,
    pub release_candidate_eligible: bool,
    pub evidence_boundary: &'static str,
}

pub struct RenderedTitleTranslationDraft {
    pub report: TitleTranslationDraftReport,
    pub(crate) rgba: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VisibleComponent {
    pub component_id: String,
    pub bounds: PixelBounds,
    pub pixels: usize,
}

pub fn render_title_translation_draft(
    source_png: &Path,
    asset: &TitleDraftAsset,
    output_png: &Path,
    japanese_source_mask_png: &Path,
    protected_nontext_mask_png: &Path,
    protected_english_mask_png: &Path,
) -> Result<RenderedTitleTranslationDraft> {
    ensure!(asset.schema == 1, "unsupported title draft asset schema");
    ensure!(
        asset.expected_text == "뿌요뿌요",
        "title draft expected text differs from the approved project term"
    );
    ensure!(
        asset.review_status == "pre_human_review",
        "title draft must remain pre-human-review"
    );
    verify_hashed_asset(&asset.generated_source)?;
    verify_hashed_asset(&asset.transparent_master)?;
    verify_hashed_asset(&asset.japanese_source_mask)?;
    verify_hashed_asset(&asset.protected_nontext_mask)?;
    verify_hashed_asset(&asset.protected_english_mask)?;

    let source = read_rgba_png(source_png)?;
    let master = read_rgba_png(Path::new(&asset.transparent_master.path))?;
    ensure!(
        source.width == 768 && source.height == 256,
        "title source surface is not 768x256"
    );
    let (source_visible_components, component_labels) = visible_components(&source)?;
    let japanese_source_mask = select_component_mask(
        &source_visible_components,
        &component_labels,
        &asset.japanese_source_components,
        source.width,
        source.height,
        "Japanese source",
    )?;
    let protected_nontext_mask = select_component_mask(
        &source_visible_components,
        &component_labels,
        &asset.protected_nontext_components,
        source.width,
        source.height,
        "protected non-text",
    )?;
    let protected_english_mask = select_component_mask(
        &source_visible_components,
        &component_labels,
        &asset.protected_english_components,
        source.width,
        source.height,
        "protected English",
    )?;
    ensure!(
        japanese_source_mask
            .iter()
            .enumerate()
            .all(|(index, selected)| !*selected || !protected_nontext_mask[index]),
        "Japanese and protected non-text component masks overlap"
    );
    let master_alpha_bounds = alpha_bounds(&master)?;
    let output = compose(
        &source,
        &master,
        master_alpha_bounds,
        &japanese_source_mask,
        &protected_nontext_mask,
    )?;
    let (changed_pixels, changed_pixels_outside) = changed_pixel_counts(&source, &output)?;
    let changed_protected_nontext_pixels =
        changed_masked_pixel_count(&source, &output, &protected_nontext_mask)?;
    let changed_protected_english_pixels =
        changed_masked_pixel_count(&source, &output, &protected_english_mask)?;
    let source_color_count = unique_color_count(&source);
    let japanese_source_color_count = unique_masked_color_count(&source, &japanese_source_mask)?;
    let changed_output_color_count = unique_changed_color_count(&source, &output)?;
    ensure!(
        changed_pixels > 0,
        "title draft did not change a source pixel"
    );
    ensure!(
        changed_pixels_outside == 0,
        "title draft changed protected pixels outside the editable bounds"
    );
    ensure!(
        changed_protected_nontext_pixels == 0,
        "title draft changed protected non-text components"
    );
    ensure!(
        changed_protected_english_pixels == 0,
        "title draft changed protected English components"
    );
    let output_colors_not_in_source = colors_not_in_source(&source, &output);
    ensure!(
        output_colors_not_in_source == 0,
        "title draft uses colors outside the exact source surface"
    );

    write_mask_png(
        japanese_source_mask_png,
        source.width,
        source.height,
        &japanese_source_mask,
    )?;
    write_mask_png(
        protected_nontext_mask_png,
        source.width,
        source.height,
        &protected_nontext_mask,
    )?;
    write_mask_png(
        protected_english_mask_png,
        source.width,
        source.height,
        &protected_english_mask,
    )?;
    if let Some(parent) = output_png.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    write_rgba_png(
        output_png,
        u32::try_from(output.width)?,
        u32::try_from(output.height)?,
        &output.pixels,
    )?;
    let source_png_bytes =
        fs::read(source_png).with_context(|| format!("failed to hash {}", source_png.display()))?;
    let output_png_bytes =
        fs::read(output_png).with_context(|| format!("failed to hash {}", output_png.display()))?;
    let japanese_source_mask_asset = hashed_output_asset(japanese_source_mask_png)?;
    let protected_nontext_mask_asset = hashed_output_asset(protected_nontext_mask_png)?;
    let protected_english_mask_asset = hashed_output_asset(protected_english_mask_png)?;
    ensure!(
        japanese_source_mask_asset.sha256 == asset.japanese_source_mask.sha256,
        "generated Japanese source mask differs from the declared asset"
    );
    ensure!(
        protected_nontext_mask_asset.sha256 == asset.protected_nontext_mask.sha256,
        "generated protected non-text mask differs from the declared asset"
    );
    ensure!(
        protected_english_mask_asset.sha256 == asset.protected_english_mask.sha256,
        "generated protected English mask differs from the declared asset"
    );
    let report = TitleTranslationDraftReport {
        schema: 1,
        candidate_id: asset.candidate_id.clone(),
        expected_text: asset.expected_text.clone(),
        source_png_sha256: sha256(&source_png_bytes),
        transparent_master_sha256: asset.transparent_master.sha256.clone(),
        output_png_sha256: sha256(&output_png_bytes),
        width: u32::try_from(output.width)?,
        height: u32::try_from(output.height)?,
        editable_bounds: EDITABLE_BOUNDS,
        lettering_placement: LETTERING_PLACEMENT,
        master_alpha_bounds,
        source_visible_components,
        japanese_source_mask: japanese_source_mask_asset,
        protected_nontext_mask: protected_nontext_mask_asset,
        protected_english_mask: protected_english_mask_asset,
        changed_pixels,
        changed_pixels_outside_editable_bounds: changed_pixels_outside,
        changed_protected_nontext_pixels,
        changed_protected_english_pixels,
        source_color_count,
        japanese_source_color_count,
        draft_quantization_color_limit: DRAFT_QUANTIZATION_COLOR_LIMIT,
        changed_output_color_count,
        output_colors_not_in_source,
        review_status: "pre_human_review",
        release_candidate_eligible: false,
        evidence_boundary: "the exact-source preview keeps pixels outside one bounded Japanese-logo rectangle and every declared non-text and English source component byte-identical in RGBA, and quantizes changed pixels to colors already used by the source surface; it does not prove exact Korean spelling, palette-index identity, CPX insertion, runtime display, human art approval, or release-candidate eligibility",
    };
    Ok(RenderedTitleTranslationDraft {
        report,
        rgba: output.pixels,
    })
}

fn visible_components(image: &RgbaImage) -> Result<(Vec<VisibleComponent>, Vec<Option<usize>>)> {
    let mut visited = vec![false; image.width * image.height];
    let mut labels = vec![None; image.width * image.height];
    let mut components = Vec::new();
    for start_y in 0..image.height {
        for start_x in 0..image.width {
            let start = start_y * image.width + start_x;
            if visited[start] || image.pixels[start * 4 + 3] == 0 {
                continue;
            }
            let mut pending = vec![(start_x, start_y)];
            visited[start] = true;
            let component_index = components.len();
            let mut left = start_x;
            let mut top = start_y;
            let mut right = start_x + 1;
            let mut bottom = start_y + 1;
            let mut pixels = 0_usize;
            while let Some((x, y)) = pending.pop() {
                labels[y * image.width + x] = Some(component_index);
                pixels += 1;
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
                for neighbor_y in y.saturating_sub(1)..=(y + 1).min(image.height - 1) {
                    for neighbor_x in x.saturating_sub(1)..=(x + 1).min(image.width - 1) {
                        let neighbor = neighbor_y * image.width + neighbor_x;
                        if visited[neighbor] || image.pixels[neighbor * 4 + 3] == 0 {
                            continue;
                        }
                        visited[neighbor] = true;
                        pending.push((neighbor_x, neighbor_y));
                    }
                }
            }
            components.push(VisibleComponent {
                component_id: String::new(),
                bounds: PixelBounds {
                    x: u32::try_from(left)?,
                    y: u32::try_from(top)?,
                    width: u32::try_from(right - left)?,
                    height: u32::try_from(bottom - top)?,
                },
                pixels,
            });
        }
    }
    for (index, component) in components.iter_mut().enumerate() {
        component.component_id = format!("visible-{index:03}");
    }
    Ok((components, labels))
}

fn select_component_mask(
    components: &[VisibleComponent],
    labels: &[Option<usize>],
    selected_ids: &[String],
    width: usize,
    height: usize,
    label: &str,
) -> Result<Vec<bool>> {
    ensure!(
        !selected_ids.is_empty(),
        "{label} component selection is empty"
    );
    let selected = selected_ids
        .iter()
        .map(|id| {
            components
                .iter()
                .position(|component| component.component_id == *id)
                .with_context(|| format!("{label} component does not exist: {id}"))
        })
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    ensure!(
        selected.len() == selected_ids.len(),
        "{label} component selection contains duplicates"
    );
    ensure!(
        labels.len() == width * height,
        "visible component label dimensions changed"
    );
    Ok(labels
        .iter()
        .map(|component| component.is_some_and(|index| selected.contains(&index)))
        .collect())
}

fn alpha_bounds(image: &RgbaImage) -> Result<PixelBounds> {
    let mut left = image.width;
    let mut top = image.height;
    let mut right = 0_usize;
    let mut bottom = 0_usize;
    for y in 0..image.height {
        for x in 0..image.width {
            if image.pixels[(y * image.width + x) * 4 + 3] == 0 {
                continue;
            }
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    ensure!(
        left < right && top < bottom,
        "title master has no visible pixels"
    );
    Ok(PixelBounds {
        x: u32::try_from(left)?,
        y: u32::try_from(top)?,
        width: u32::try_from(right - left)?,
        height: u32::try_from(bottom - top)?,
    })
}

fn compose(
    source: &RgbaImage,
    master: &RgbaImage,
    master_bounds: PixelBounds,
    japanese_source_mask: &[bool],
    protected_nontext_mask: &[bool],
) -> Result<RgbaImage> {
    validate_bounds(source, EDITABLE_BOUNDS, "editable")?;
    validate_bounds(source, LETTERING_PLACEMENT, "lettering placement")?;
    ensure!(
        contains(EDITABLE_BOUNDS, LETTERING_PLACEMENT),
        "lettering placement exceeds the editable bounds"
    );
    ensure!(
        protected_nontext_mask.len() == source.width * source.height,
        "protected non-text mask dimensions changed"
    );
    let mut output = source.clone();
    let background = most_common_pixel(source, EDITABLE_BOUNDS)?;
    fill(
        &mut output,
        EDITABLE_BOUNDS,
        background,
        protected_nontext_mask,
    )?;

    let fitted = fit_inside(master_bounds, LETTERING_PLACEMENT)?;
    for y in 0..fitted.height {
        for x in 0..fitted.width {
            let source_x = u64::from(x) * u64::from(master_bounds.width) / u64::from(fitted.width)
                + u64::from(master_bounds.x);
            let source_y = u64::from(y) * u64::from(master_bounds.height)
                / u64::from(fitted.height)
                + u64::from(master_bounds.y);
            let master_offset =
                (usize::try_from(source_y)? * master.width + usize::try_from(source_x)?) * 4;
            let target_offset = (usize::try_from(fitted.y + y)? * output.width
                + usize::try_from(fitted.x + x)?)
                * 4;
            let target_pixel = target_offset / 4;
            if protected_nontext_mask[target_pixel] {
                continue;
            }
            alpha_over(
                &mut output.pixels[target_offset..target_offset + 4],
                &master.pixels[master_offset..master_offset + 4],
            );
        }
    }
    quantize_changed_pixels_to_source(source, &mut output, japanese_source_mask, background)?;
    Ok(output)
}

fn validate_bounds(image: &RgbaImage, bounds: PixelBounds, label: &str) -> Result<()> {
    ensure!(
        bounds.width > 0
            && bounds.height > 0
            && u64::from(bounds.x) + u64::from(bounds.width) <= image.width as u64
            && u64::from(bounds.y) + u64::from(bounds.height) <= image.height as u64,
        "{label} bounds exceed the image"
    );
    Ok(())
}

fn contains(outer: PixelBounds, inner: PixelBounds) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn fit_inside(source: PixelBounds, target: PixelBounds) -> Result<PixelBounds> {
    let width_limited_height =
        u64::from(target.width) * u64::from(source.height) / u64::from(source.width);
    let (width, height) = if width_limited_height <= u64::from(target.height) {
        (u64::from(target.width), width_limited_height)
    } else {
        (
            u64::from(target.height) * u64::from(source.width) / u64::from(source.height),
            u64::from(target.height),
        )
    };
    ensure!(
        width > 0 && height > 0,
        "title master does not fit the placement"
    );
    let width = u32::try_from(width)?;
    let height = u32::try_from(height)?;
    Ok(PixelBounds {
        x: target.x + (target.width - width) / 2,
        y: target.y + (target.height - height) / 2,
        width,
        height,
    })
}

fn most_common_pixel(image: &RgbaImage, bounds: PixelBounds) -> Result<[u8; 4]> {
    let mut counts = std::collections::BTreeMap::<[u8; 4], usize>::new();
    for y in bounds.y..bounds.y + bounds.height {
        for x in bounds.x..bounds.x + bounds.width {
            let offset = (usize::try_from(y)? * image.width + usize::try_from(x)?) * 4;
            let pixel: [u8; 4] = image.pixels[offset..offset + 4].try_into()?;
            *counts.entry(pixel).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(pixel, count)| (*count, *pixel))
        .map(|(pixel, _)| pixel)
        .context("editable title bounds have no pixels")
}

fn fill(
    image: &mut RgbaImage,
    bounds: PixelBounds,
    pixel: [u8; 4],
    protected_mask: &[bool],
) -> Result<()> {
    ensure!(
        protected_mask.len() == image.width * image.height,
        "fill protection mask dimensions changed"
    );
    for y in bounds.y..bounds.y + bounds.height {
        for x in bounds.x..bounds.x + bounds.width {
            let pixel_index = usize::try_from(y)? * image.width + usize::try_from(x)?;
            if protected_mask[pixel_index] {
                continue;
            }
            let offset = pixel_index * 4;
            image.pixels[offset..offset + 4].copy_from_slice(&pixel);
        }
    }
    Ok(())
}

fn quantize_changed_pixels_to_source(
    source: &RgbaImage,
    output: &mut RgbaImage,
    japanese_source_mask: &[bool],
    background: [u8; 4],
) -> Result<()> {
    ensure!(
        source.width == output.width && source.height == output.height,
        "quantization image dimensions changed"
    );
    ensure!(
        japanese_source_mask.len() == source.width * source.height,
        "Japanese quantization mask dimensions changed"
    );
    let mut frequencies = std::collections::BTreeMap::<[u8; 4], usize>::new();
    for (pixel, selected) in source
        .pixels
        .chunks_exact(4)
        .zip(japanese_source_mask.iter())
    {
        if *selected {
            let pixel = <[u8; 4]>::try_from(pixel).expect("four-byte RGBA pixel");
            *frequencies.entry(pixel).or_default() += 1;
        }
    }
    frequencies.entry(background).or_insert(usize::MAX);
    let mut ranked = frequencies.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|(left_color, left_count), (right_color, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_color.cmp(right_color))
    });
    let palette = ranked
        .into_iter()
        .take(DRAFT_QUANTIZATION_COLOR_LIMIT)
        .map(|(color, _)| color)
        .collect::<Vec<_>>();
    ensure!(!palette.is_empty(), "title source has no colors");
    for (source_pixel, output_pixel) in source
        .pixels
        .chunks_exact(4)
        .zip(output.pixels.chunks_exact_mut(4))
    {
        if source_pixel == output_pixel {
            continue;
        }
        let candidate: [u8; 4] = output_pixel.try_into()?;
        let nearest = palette
            .iter()
            .min_by_key(|color| rgba_distance(candidate, **color))
            .context("title source palette is empty")?;
        output_pixel.copy_from_slice(nearest);
    }
    Ok(())
}

fn rgba_distance(left: [u8; 4], right: [u8; 4]) -> u64 {
    left.into_iter()
        .zip(right)
        .enumerate()
        .map(|(channel, (left, right))| {
            let difference = i64::from(left) - i64::from(right);
            let weight = if channel == 3 { 4 } else { 1 };
            (difference * difference * weight) as u64
        })
        .sum()
}

fn colors_not_in_source(source: &RgbaImage, output: &RgbaImage) -> usize {
    let source_colors = source
        .pixels
        .chunks_exact(4)
        .map(|pixel| <[u8; 4]>::try_from(pixel).expect("four-byte RGBA pixel"))
        .collect::<std::collections::BTreeSet<_>>();
    output
        .pixels
        .chunks_exact(4)
        .filter(|pixel| {
            let pixel = <[u8; 4]>::try_from(*pixel).expect("four-byte RGBA pixel");
            !source_colors.contains(&pixel)
        })
        .count()
}

fn unique_color_count(image: &RgbaImage) -> usize {
    image
        .pixels
        .chunks_exact(4)
        .map(|pixel| <[u8; 4]>::try_from(pixel).expect("four-byte RGBA pixel"))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn unique_masked_color_count(image: &RgbaImage, mask: &[bool]) -> Result<usize> {
    ensure!(
        mask.len() == image.width * image.height,
        "masked color census dimensions changed"
    );
    Ok(mask
        .iter()
        .enumerate()
        .filter(|(_, selected)| **selected)
        .map(|(index, _)| {
            <[u8; 4]>::try_from(&image.pixels[index * 4..index * 4 + 4])
                .expect("four-byte RGBA pixel")
        })
        .collect::<std::collections::BTreeSet<_>>()
        .len())
}

fn unique_changed_color_count(before: &RgbaImage, after: &RgbaImage) -> Result<usize> {
    ensure!(
        before.width == after.width && before.height == after.height,
        "changed color census dimensions changed"
    );
    Ok(before
        .pixels
        .chunks_exact(4)
        .zip(after.pixels.chunks_exact(4))
        .filter(|(source, replacement)| source != replacement)
        .map(|(_, replacement)| <[u8; 4]>::try_from(replacement).expect("four-byte RGBA pixel"))
        .collect::<std::collections::BTreeSet<_>>()
        .len())
}

fn alpha_over(destination: &mut [u8], source: &[u8]) {
    let source_alpha = u32::from(source[3]);
    let inverse = 255 - source_alpha;
    let destination_alpha = u32::from(destination[3]);
    let output_alpha = source_alpha + (destination_alpha * inverse + 127) / 255;
    if output_alpha == 0 {
        destination.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    for channel in 0..3 {
        let premultiplied = u32::from(source[channel]) * source_alpha
            + (u32::from(destination[channel]) * destination_alpha * inverse + 127) / 255;
        destination[channel] = ((premultiplied + output_alpha / 2) / output_alpha) as u8;
    }
    destination[3] = output_alpha as u8;
}

fn changed_pixel_counts(before: &RgbaImage, after: &RgbaImage) -> Result<(usize, usize)> {
    ensure!(
        before.width == after.width && before.height == after.height,
        "title draft dimensions changed"
    );
    let mut changed = 0_usize;
    let mut outside = 0_usize;
    for y in 0..before.height {
        for x in 0..before.width {
            let offset = (y * before.width + x) * 4;
            if before.pixels[offset..offset + 4] == after.pixels[offset..offset + 4] {
                continue;
            }
            changed += 1;
            if !contains(
                EDITABLE_BOUNDS,
                PixelBounds {
                    x: u32::try_from(x)?,
                    y: u32::try_from(y)?,
                    width: 1,
                    height: 1,
                },
            ) {
                outside += 1;
            }
        }
    }
    Ok((changed, outside))
}

fn changed_masked_pixel_count(
    before: &RgbaImage,
    after: &RgbaImage,
    mask: &[bool],
) -> Result<usize> {
    ensure!(
        before.width == after.width
            && before.height == after.height
            && mask.len() == before.width * before.height,
        "masked pixel comparison dimensions changed"
    );
    Ok(mask
        .iter()
        .enumerate()
        .filter(|(index, selected)| {
            if !**selected {
                return false;
            }
            let offset = *index * 4;
            before.pixels[offset..offset + 4] != after.pixels[offset..offset + 4]
        })
        .count())
}

#[cfg(test)]
#[path = "title_draft_tests.rs"]
mod title_draft_tests;
