use anyhow::{Context, Result, ensure};

use super::super::static_regions::StaticRegionDeclaration;
use super::super::{GraphicsTreatmentProfile, PixelBounds};
use super::font::LoadedStaticUiFont;
use super::model::StaticUiLineAlignment;
use crate::bitmap_font::{GlyphPixel, RasterizedLine, rasterize_line};

#[derive(Clone, Copy)]
pub(crate) struct SurfaceDimensions {
    pub(crate) width: usize,
    pub(crate) height: usize,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct LineLayout {
    pub(crate) maximum_pixel_size: Option<u32>,
    pub(crate) alignment: Option<StaticUiLineAlignment>,
}

impl LineLayout {
    pub(crate) fn horizontal_position(
        self,
        source_alignment: StaticUiLineAlignment,
        left: usize,
        width: usize,
        text_width: usize,
    ) -> (StaticUiLineAlignment, usize) {
        let alignment = self.alignment.unwrap_or(source_alignment);
        let x = match alignment {
            StaticUiLineAlignment::Left => left,
            StaticUiLineAlignment::Center => left + (width - text_width) / 2,
            StaticUiLineAlignment::Right => left + width - text_width,
        };
        (alignment, x)
    }
}

pub(crate) struct FittedRegion {
    pub(crate) pixel_size: u32,
    pub(crate) lines: Vec<FittedLine>,
}

pub(crate) struct FittedLine {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) alignment: StaticUiLineAlignment,
    pub(crate) raster: RasterizedLine,
}

pub(super) fn fit_region_copies(
    loaded: &LoadedStaticUiFont,
    korean_text: &str,
    declaration: &StaticRegionDeclaration,
    surface_width: usize,
    surface_height: usize,
    source_mask: &[bool],
) -> Result<Option<Vec<FittedRegion>>> {
    let copy_bounds =
        repeated_horizontal_copy_bounds(source_mask, surface_width, surface_height, declaration)?;
    if copy_bounds.len() == 1 {
        return Ok(fit_region_content(
            loaded,
            korean_text,
            declaration,
            surface_width,
            surface_height,
            source_mask,
        )?
        .map(|fitted| vec![fitted]));
    }

    let words = korean_text.split_whitespace().collect::<Vec<_>>();
    let mut candidates = vec![korean_text.to_string()];
    for split in 1..words.len() {
        candidates.push(format!(
            "{}\n{}",
            words[..split].join(" "),
            words[split..].join(" ")
        ));
    }
    let mut best = None::<(u32, Vec<FittedRegion>)>;
    for candidate in candidates {
        let mut copies = Vec::with_capacity(copy_bounds.len());
        for bounds in &copy_bounds {
            let Some(fitted) = fit_region_lines(
                loaded,
                &candidate,
                *bounds,
                source_mask,
                SurfaceDimensions {
                    width: surface_width,
                    height: surface_height,
                },
                LineLayout {
                    maximum_pixel_size: declaration.maximum_pixel_size,
                    alignment: declaration.text_alignment,
                },
                &[],
            )?
            else {
                copies.clear();
                break;
            };
            copies.push(fitted);
        }
        if copies.len() != copy_bounds.len() {
            continue;
        }
        let score = copies
            .iter()
            .map(|copy| copy.pixel_size)
            .min()
            .expect("repeated functional UI region has copies");
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, copies));
        }
    }
    Ok(best.map(|(_, copies)| copies))
}

fn repeated_horizontal_copy_bounds(
    source_mask: &[bool],
    surface_width: usize,
    surface_height: usize,
    declaration: &StaticRegionDeclaration,
) -> Result<Vec<PixelBounds>> {
    ensure!(
        source_mask.len() == surface_width * surface_height,
        "functional UI repeated-copy mask dimensions changed"
    );
    let copy_count = usize::from(declaration.horizontal_copy_count.unwrap_or(1));
    ensure!(
        (1..=8).contains(&copy_count),
        "functional UI horizontal copy count is outside 1..=8: {}",
        declaration.region_id
    );
    if copy_count == 1 {
        return Ok(vec![declaration.bounds]);
    }
    ensure!(
        declaration.protected_subregions.is_empty()
            && !declaration.source_text.contains('\n')
            && usize::try_from(declaration.bounds.width)? >= copy_count,
        "repeated functional UI region has an incompatible layout declaration: {}",
        declaration.region_id
    );
    let left = usize::try_from(declaration.bounds.x)?;
    let top = usize::try_from(declaration.bounds.y)?;
    let right = usize::try_from(declaration.bounds.x + declaration.bounds.width)?;
    let bottom = usize::try_from(declaration.bounds.y + declaration.bounds.height)?;
    ensure!(
        left < right && top < bottom && right <= surface_width && bottom <= surface_height,
        "functional UI repeated-copy bounds exceed the surface"
    );
    (0..copy_count)
        .map(|copy_index| {
            let cell_start = left + (right - left) * copy_index / copy_count;
            let cell_end = left + (right - left) * (copy_index + 1) / copy_count;
            let x_start = (cell_start..cell_end)
                .find(|x| (top..bottom).any(|y| source_mask[y * surface_width + *x]))
                .context("repeated functional UI copy has no left pixel")?;
            let x_end = (cell_start..cell_end)
                .rev()
                .find(|x| (top..bottom).any(|y| source_mask[y * surface_width + *x]))
                .map(|x| x + 1)
                .context("repeated functional UI copy has no right pixel")?;
            let y_start = (top..bottom)
                .find(|y| (x_start..x_end).any(|x| source_mask[*y * surface_width + x]))
                .context("repeated functional UI copy has no top pixel")?;
            let y_end = (top..bottom)
                .rev()
                .find(|y| (x_start..x_end).any(|x| source_mask[*y * surface_width + x]))
                .map(|y| y + 1)
                .context("repeated functional UI copy has no bottom pixel")?;
            Ok(PixelBounds {
                x: u32::try_from(x_start)?,
                y: u32::try_from(y_start)?,
                width: u32::try_from(x_end - x_start)?,
                height: u32::try_from(y_end - y_start)?,
            })
        })
        .collect()
}

pub(crate) fn fit_region_lines(
    loaded: &LoadedStaticUiFont,
    korean_text: &str,
    bounds: PixelBounds,
    source_mask: &[bool],
    dimensions: SurfaceDimensions,
    layout: LineLayout,
    protected_subregions: &[PixelBounds],
) -> Result<Option<FittedRegion>> {
    let SurfaceDimensions {
        width: surface_width,
        height: surface_height,
    } = dimensions;
    let lines = korean_text.split('\n').collect::<Vec<_>>();
    ensure!(
        !lines.is_empty() && lines.iter().all(|line| !line.trim().is_empty()),
        "functional UI Korean text contains an empty line"
    );
    let bounds_x = usize::try_from(bounds.x)?;
    let bounds_y = usize::try_from(bounds.y)?;
    let bounds_width = usize::try_from(bounds.width)?;
    let bounds_height = usize::try_from(bounds.height)?;
    ensure!(
        bounds_x + bounds_width <= surface_width
            && bounds_y + bounds_height <= surface_height
            && source_mask.len() == surface_width * surface_height,
        "functional UI bounds exceed their surface"
    );
    let maximum_pixel_size = maximum_render_pixel_size(loaded, layout.maximum_pixel_size)?;
    for pixel_size in (loaded.config.rasterization.minimum_pixel_size..=maximum_pixel_size).rev() {
        let rasters = lines
            .iter()
            .map(|line| {
                rasterize_line(
                    &loaded.font,
                    line,
                    pixel_size as f32,
                    loaded.config.rasterization.coverage_threshold,
                    loaded.config.rasterization.outline_radius,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let mut fitted = Vec::with_capacity(lines.len());
        let mut fits = true;
        for (line_index, raster) in rasters.into_iter().enumerate() {
            let cell_top = bounds_y + bounds_height * line_index / lines.len();
            let cell_bottom = bounds_y + bounds_height * (line_index + 1) / lines.len();
            let cell_height = cell_bottom - cell_top;
            if raster.width > bounds_width || raster.height > cell_height {
                fits = false;
                break;
            }
            let (alignment, x) = layout.horizontal_position(
                infer_alignment(
                    source_mask,
                    surface_width,
                    bounds_x,
                    bounds_width,
                    cell_top,
                    cell_bottom,
                ),
                bounds_x,
                bounds_width,
                raster.width,
            );
            let y = cell_top + (cell_height - raster.height) / 2;
            if raster_overlaps_protected_subregion(&raster, x, y, protected_subregions) {
                fits = false;
                break;
            }
            fitted.push(FittedLine {
                x,
                y,
                alignment,
                raster,
            });
        }
        if fits {
            return Ok(Some(FittedRegion {
                pixel_size,
                lines: fitted,
            }));
        }
    }
    Ok(None)
}

fn maximum_render_pixel_size(loaded: &LoadedStaticUiFont, requested: Option<u32>) -> Result<u32> {
    let maximum = requested.unwrap_or(loaded.config.rasterization.maximum_pixel_size);
    ensure!(
        maximum >= loaded.config.rasterization.minimum_pixel_size
            && maximum <= loaded.config.rasterization.maximum_pixel_size,
        "static text maximum pixel size falls outside the configured font range"
    );
    Ok(maximum)
}

fn raster_overlaps_protected_subregion(
    raster: &RasterizedLine,
    x: usize,
    y: usize,
    protected_subregions: &[PixelBounds],
) -> bool {
    raster
        .pixels
        .iter()
        .enumerate()
        .filter(|(_, role)| **role != GlyphPixel::Transparent)
        .any(|(pixel, _)| {
            let pixel_x = x + pixel % raster.width;
            let pixel_y = y + pixel / raster.width;
            protected_subregions.iter().any(|protected| {
                let left = usize::try_from(protected.x).expect("protected x fits usize");
                let top = usize::try_from(protected.y).expect("protected y fits usize");
                let right = usize::try_from(protected.x + protected.width)
                    .expect("protected right fits usize");
                let bottom = usize::try_from(protected.y + protected.height)
                    .expect("protected bottom fits usize");
                pixel_x >= left && pixel_x < right && pixel_y >= top && pixel_y < bottom
            })
        })
}

fn fit_region_content(
    loaded: &LoadedStaticUiFont,
    korean_text: &str,
    declaration: &StaticRegionDeclaration,
    surface_width: usize,
    surface_height: usize,
    source_mask: &[bool],
) -> Result<Option<FittedRegion>> {
    if declaration.protected_subregions.is_empty() {
        return fit_region_lines(
            loaded,
            korean_text,
            declaration
                .artwork_composition_bounds
                .unwrap_or(declaration.bounds),
            source_mask,
            SurfaceDimensions {
                width: surface_width,
                height: surface_height,
            },
            LineLayout {
                maximum_pixel_size: declaration.maximum_pixel_size,
                alignment: declaration.text_alignment,
            },
            &[],
        );
    }
    if declaration.treatment_profile == GraphicsTreatmentProfile::StylizedDisplayLettering {
        return fit_region_lines(
            loaded,
            korean_text,
            declaration.bounds,
            source_mask,
            SurfaceDimensions {
                width: surface_width,
                height: surface_height,
            },
            LineLayout {
                maximum_pixel_size: declaration.maximum_pixel_size,
                alignment: declaration.text_alignment,
            },
            &declaration.protected_subregions,
        );
    }
    ensure!(
        declaration.protected_subregions.len() == 1 && !korean_text.contains('\n'),
        "functional UI fixed-token layout requires one protected subregion and one line: {}",
        declaration.region_id
    );
    let token_start = korean_text
        .find('[')
        .context("functional UI fixed-token Korean text has no opening bracket")?;
    let token_end = korean_text[token_start..]
        .find(']')
        .map(|offset| token_start + offset + 1)
        .context("functional UI fixed-token Korean text has no closing bracket")?;
    let token = &korean_text[token_start..token_end];
    ensure!(
        token.is_ascii()
            && declaration.source_text.contains(token)
            && token_start > 0
            && token_end < korean_text.len(),
        "functional UI fixed token is not preserved verbatim: {}",
        declaration.region_id
    );
    let prefix = korean_text[..token_start].trim_end();
    let suffix = korean_text[token_end..].trim_start();
    ensure!(
        !prefix.is_empty() && !suffix.is_empty(),
        "functional UI fixed token lacks a Korean prefix or suffix"
    );
    let bounds = declaration.bounds;
    let fixed = declaration.protected_subregions[0];
    ensure!(
        fixed.x >= bounds.x
            && fixed.y >= bounds.y
            && fixed.x + fixed.width <= bounds.x + bounds.width
            && fixed.y + fixed.height <= bounds.y + bounds.height,
        "functional UI fixed token falls outside its declared region"
    );
    let gap = 2_usize;
    let bounds_left = usize::try_from(bounds.x)?;
    let bounds_top = usize::try_from(bounds.y)?;
    let bounds_right = usize::try_from(bounds.x + bounds.width)?;
    let bounds_height = usize::try_from(bounds.height)?;
    let fixed_left = usize::try_from(fixed.x)?;
    let fixed_right = usize::try_from(fixed.x + fixed.width)?;
    let left_width = fixed_left.saturating_sub(bounds_left + gap);
    let right_width = bounds_right.saturating_sub(fixed_right + gap);
    let maximum_pixel_size = maximum_render_pixel_size(loaded, declaration.maximum_pixel_size)?;
    for pixel_size in (loaded.config.rasterization.minimum_pixel_size..=maximum_pixel_size).rev() {
        let prefix_raster = rasterize_line(
            &loaded.font,
            prefix,
            pixel_size as f32,
            loaded.config.rasterization.coverage_threshold,
            loaded.config.rasterization.outline_radius,
        )?;
        let suffix_raster = rasterize_line(
            &loaded.font,
            suffix,
            pixel_size as f32,
            loaded.config.rasterization.coverage_threshold,
            loaded.config.rasterization.outline_radius,
        )?;
        if prefix_raster.width > left_width
            || suffix_raster.width > right_width
            || prefix_raster.height > bounds_height
            || suffix_raster.height > bounds_height
        {
            continue;
        }
        return Ok(Some(FittedRegion {
            pixel_size,
            lines: vec![
                FittedLine {
                    x: fixed_left - gap - prefix_raster.width,
                    y: bounds_top + (bounds_height - prefix_raster.height) / 2,
                    alignment: StaticUiLineAlignment::Right,
                    raster: prefix_raster,
                },
                FittedLine {
                    x: fixed_right + gap,
                    y: bounds_top + (bounds_height - suffix_raster.height) / 2,
                    alignment: StaticUiLineAlignment::Left,
                    raster: suffix_raster,
                },
            ],
        }));
    }
    Ok(None)
}

pub(super) fn infer_alignment(
    source_mask: &[bool],
    surface_width: usize,
    bounds_x: usize,
    bounds_width: usize,
    top: usize,
    bottom: usize,
) -> StaticUiLineAlignment {
    let selected_x = (top..bottom)
        .flat_map(|y| {
            (bounds_x..bounds_x + bounds_width).filter(move |x| source_mask[y * surface_width + *x])
        })
        .collect::<Vec<_>>();
    let Some(min_x) = selected_x.iter().min().copied() else {
        return StaticUiLineAlignment::Center;
    };
    let max_x = selected_x.iter().max().copied().unwrap_or(min_x);
    let left_margin = min_x - bounds_x;
    let right_margin = bounds_x + bounds_width - 1 - max_x;
    let close = 4_usize.max(bounds_width / 20);
    match (left_margin <= close, right_margin <= close) {
        (true, false) => StaticUiLineAlignment::Left,
        (false, true) => StaticUiLineAlignment::Right,
        _ => StaticUiLineAlignment::Center,
    }
}

pub(super) fn visible_unselected_pixels(
    source_rgba: &[u8],
    surface_width: usize,
    surface_height: usize,
    bounds: PixelBounds,
    source_mask: &[bool],
) -> Result<usize> {
    ensure!(
        source_rgba.len() == surface_width * surface_height * 4
            && source_mask.len() == surface_width * surface_height,
        "functional UI source-mask dimensions changed"
    );
    let mut count = 0;
    for y in usize::try_from(bounds.y)?..usize::try_from(bounds.y + bounds.height)? {
        for x in usize::try_from(bounds.x)?..usize::try_from(bounds.x + bounds.width)? {
            let pixel = y * surface_width + x;
            if source_rgba[pixel * 4 + 3] != 0 && !source_mask[pixel] {
                count += 1;
            }
        }
    }
    Ok(count)
}
