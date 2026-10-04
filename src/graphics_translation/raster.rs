use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use super::{HashedAsset, PixelBounds};
use crate::indexed_texture::sha256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RgbaSurface {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

pub(crate) fn read_rgba_surface(path: &Path) -> Result<RgbaSurface> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("failed to read PNG header: {}", path.display()))?;
    let output_size = reader
        .output_buffer_size()
        .context("PNG output buffer size is unavailable")?;
    let mut buffer = vec![0; output_size];
    let info = reader
        .next_frame(&mut buffer)
        .with_context(|| format!("failed to decode PNG: {}", path.display()))?;
    ensure!(
        info.bit_depth == png::BitDepth::Eight,
        "PNG is not eight-bit: {}",
        path.display()
    );
    let width = usize::try_from(info.width)?;
    let height = usize::try_from(info.height)?;
    let source = &buffer[..info.buffer_size()];
    let mut pixels = Vec::with_capacity(width * height * 4);
    match info.color_type {
        png::ColorType::Rgba => pixels.extend_from_slice(source),
        png::ColorType::Rgb => {
            for pixel in source.chunks_exact(3) {
                pixels.extend_from_slice(&[pixel[0], pixel[1], pixel[2], u8::MAX]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in source.chunks_exact(2) {
                pixels.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Grayscale => {
            for value in source {
                pixels.extend_from_slice(&[*value, *value, *value, u8::MAX]);
            }
        }
        png::ColorType::Indexed => bail!("expanded PNG unexpectedly remained indexed"),
    }
    ensure!(
        pixels.len() == width * height * 4,
        "decoded PNG byte length differs from its dimensions"
    );
    Ok(RgbaSurface {
        width,
        height,
        pixels,
    })
}

pub(crate) fn read_binary_mask(
    path: &Path,
    expected_width: usize,
    expected_height: usize,
    context: &str,
) -> Result<Vec<bool>> {
    let mask = read_rgba_surface(path)?;
    ensure!(
        mask.width == expected_width && mask.height == expected_height,
        "binary mask dimensions differ for {context}"
    );
    mask.pixels
        .chunks_exact(4)
        .map(|pixel| {
            ensure!(
                pixel[0] == pixel[1]
                    && pixel[1] == pixel[2]
                    && matches!(pixel[0], 0 | u8::MAX)
                    && pixel[3] == u8::MAX,
                "binary mask contains a non-binary pixel for {context}"
            );
            Ok(pixel[0] == u8::MAX)
        })
        .collect()
}

pub(crate) fn write_binary_mask(
    path: &Path,
    width: usize,
    height: usize,
    selected_pixels: &[bool],
) -> Result<()> {
    ensure!(
        selected_pixels.len() == width * height,
        "mask dimensions changed"
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        u32::try_from(width)?,
        u32::try_from(height)?,
    );
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .with_context(|| format!("failed to write PNG header: {}", path.display()))?;
    let pixels = selected_pixels
        .iter()
        .map(|selected| if *selected { u8::MAX } else { 0 })
        .collect::<Vec<_>>();
    writer
        .write_image_data(&pixels)
        .with_context(|| format!("failed to write PNG mask: {}", path.display()))?;
    Ok(())
}

pub(super) fn hashed_asset(path: &Path) -> Result<HashedAsset> {
    let bytes = fs::read(path).with_context(|| format!("failed to hash {}", path.display()))?;
    Ok(HashedAsset {
        path: path.to_string_lossy().into_owned(),
        sha256: sha256(&bytes),
    })
}

pub(crate) fn verify_hashed_asset(asset: &HashedAsset) -> Result<()> {
    let bytes = fs::read(&asset.path)
        .with_context(|| format!("failed to read graphics asset {}", asset.path))?;
    ensure!(
        sha256(&bytes) == asset.sha256,
        "graphics asset SHA-256 differs: {}",
        asset.path
    );
    Ok(())
}

pub(super) fn selected_bounds(
    mask: &[bool],
    width: usize,
    height: usize,
    context: &str,
) -> Result<PixelBounds> {
    ensure!(mask.len() == width * height, "mask dimensions changed");
    let selected = mask
        .iter()
        .enumerate()
        .filter_map(|(index, selected)| selected.then_some((index % width, index / width)))
        .collect::<Vec<_>>();
    ensure!(!selected.is_empty(), "mask is empty: {context}");
    let min_x = selected.iter().map(|(x, _)| *x).min().unwrap_or_default();
    let max_x = selected.iter().map(|(x, _)| *x).max().unwrap_or_default();
    let min_y = selected.iter().map(|(_, y)| *y).min().unwrap_or_default();
    let max_y = selected.iter().map(|(_, y)| *y).max().unwrap_or_default();
    Ok(PixelBounds {
        x: u32::try_from(min_x)?,
        y: u32::try_from(min_y)?,
        width: u32::try_from(max_x - min_x + 1)?,
        height: u32::try_from(max_y - min_y + 1)?,
    })
}

#[cfg(test)]
#[path = "raster_tests.rs"]
mod raster_tests;
