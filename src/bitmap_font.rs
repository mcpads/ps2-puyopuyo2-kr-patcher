use anyhow::{Context, Result, ensure};
use fontdue::{Font, FontSettings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GlyphPixel {
    Transparent,
    Outline,
    Fill,
}

pub(crate) struct RasterizedLine {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<GlyphPixel>,
}

pub(crate) fn load_font(bytes: Vec<u8>, context: &str) -> Result<Font> {
    Font::from_bytes(bytes, FontSettings::default())
        .map_err(|error| anyhow::anyhow!("failed to parse {context}: {error}"))
}

pub(crate) fn rasterize_line(
    font: &Font,
    text: &str,
    pixel_size: f32,
    coverage_threshold: u8,
    outline_radius: usize,
) -> Result<RasterizedLine> {
    ensure!(
        pixel_size.is_finite() && pixel_size > 0.0,
        "bitmap font size must be positive"
    );
    ensure!(!text.is_empty(), "bitmap font line is empty");
    let mut pen_x = 0.0_f32;
    let mut points = Vec::<(i32, i32)>::new();
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for character in text.chars() {
        ensure!(
            font.lookup_glyph_index(character) != 0,
            "bitmap font has no glyph for {character:?} (U+{:04X})",
            u32::from(character)
        );
        let (metrics, raster) = font.rasterize(character, pixel_size);
        let glyph_x = pen_x.round() as i32 + metrics.xmin;
        let glyph_y = -metrics.ymin - i32::try_from(metrics.height)?;
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                if raster[row * metrics.width + column] < coverage_threshold {
                    continue;
                }
                let x = glyph_x + i32::try_from(column)?;
                let y = glyph_y + i32::try_from(row)?;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + 1);
                max_y = max_y.max(y + 1);
                points.push((x, y));
            }
        }
        pen_x += metrics.advance_width;
    }
    ensure!(
        min_x < max_x && min_y < max_y,
        "bitmap font line rendered no pixels"
    );

    let width = usize::try_from(max_x - min_x)?
        .checked_add(outline_radius * 2)
        .context("bitmap font line width overflow")?;
    let height = usize::try_from(max_y - min_y)?
        .checked_add(outline_radius * 2)
        .context("bitmap font line height overflow")?;
    let mut fill = vec![false; width * height];
    for (x, y) in points {
        let x = usize::try_from(x - min_x)? + outline_radius;
        let y = usize::try_from(y - min_y)? + outline_radius;
        fill[y * width + x] = true;
    }
    let mut outline = fill.clone();
    for y in 0..height {
        for x in 0..width {
            if !fill[y * width + x] {
                continue;
            }
            let left = x.saturating_sub(outline_radius);
            let right = (x + outline_radius).min(width - 1);
            let top = y.saturating_sub(outline_radius);
            let bottom = (y + outline_radius).min(height - 1);
            for outline_y in top..=bottom {
                for outline_x in left..=right {
                    outline[outline_y * width + outline_x] = true;
                }
            }
        }
    }
    let pixels = fill
        .into_iter()
        .zip(outline)
        .map(|(fill, outline)| {
            if fill {
                GlyphPixel::Fill
            } else if outline {
                GlyphPixel::Outline
            } else {
                GlyphPixel::Transparent
            }
        })
        .collect();
    Ok(RasterizedLine {
        width,
        height,
        pixels,
    })
}
