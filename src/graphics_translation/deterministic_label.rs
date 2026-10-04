use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::static_ui::load_static_ui_font;
use crate::bitmap_font::{GlyphPixel, rasterize_line};
use crate::indexed_texture::{sha256, write_rgba_png};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeterministicLabelMasterConfiguration {
    pub schema: u32,
    pub id: String,
    pub text: String,
    pub font_id: String,
    pub font_sha256: String,
    pub pixel_size: u32,
    pub coverage_threshold: u8,
    pub outline_radius: usize,
    pub fill_rgba: [u8; 4],
    pub outline_rgba: [u8; 4],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DeterministicLabelMasterReport {
    pub schema: u32,
    pub id: String,
    pub text: String,
    pub font_id: String,
    pub font_sha256: String,
    pub pixel_size: u32,
    pub coverage_threshold: u8,
    pub outline_radius: usize,
    pub width: u32,
    pub height: u32,
    pub fill_pixels: usize,
    pub outline_pixels: usize,
    pub output_path: String,
    pub output_sha256: String,
    pub evidence_boundary: &'static str,
}

pub fn render_deterministic_label_master(
    configuration: &DeterministicLabelMasterConfiguration,
    font_configuration_path: &Path,
    font_path: &Path,
    output_path: &Path,
    refresh_existing: bool,
) -> Result<DeterministicLabelMasterReport> {
    ensure!(
        configuration.schema == 1
            && !configuration.id.trim().is_empty()
            && !configuration.text.trim().is_empty()
            && !configuration.text.contains(['\n', '\r'])
            && configuration.coverage_threshold > 0
            && configuration.outline_radius > 0
            && configuration.outline_radius <= 4
            && configuration.fill_rgba[3] == u8::MAX
            && configuration.outline_rgba[3] == u8::MAX
            && configuration.fill_rgba != configuration.outline_rgba,
        "deterministic label master configuration is incomplete"
    );
    let loaded = load_static_ui_font(font_configuration_path, font_path)?;
    ensure!(
        configuration.font_id == loaded.config.id
            && configuration.font_sha256 == loaded.config.sha256
            && configuration.pixel_size >= loaded.config.rasterization.minimum_pixel_size
            && configuration.pixel_size <= loaded.config.rasterization.maximum_pixel_size,
        "deterministic label master differs from its fixed font identity or size range"
    );
    let raster = rasterize_line(
        &loaded.font,
        &configuration.text,
        configuration.pixel_size as f32,
        configuration.coverage_threshold,
        configuration.outline_radius,
    )?;
    let mut rgba = vec![0_u8; raster.width * raster.height * 4];
    let mut fill_pixels = 0;
    let mut outline_pixels = 0;
    for (source, destination) in raster.pixels.iter().zip(rgba.chunks_exact_mut(4)) {
        match source {
            GlyphPixel::Transparent => {}
            GlyphPixel::Outline => {
                destination.copy_from_slice(&configuration.outline_rgba);
                outline_pixels += 1;
            }
            GlyphPixel::Fill => {
                destination.copy_from_slice(&configuration.fill_rgba);
                fill_pixels += 1;
            }
        }
    }
    ensure!(
        fill_pixels > 0 && outline_pixels > 0,
        "deterministic label master has no fill or outline pixels"
    );

    let staging = tempfile::tempdir().context("failed to stage deterministic label master")?;
    let staged_path = staging.path().join("master.png");
    write_rgba_png(
        &staged_path,
        u32::try_from(raster.width)?,
        u32::try_from(raster.height)?,
        &rgba,
    )?;
    let staged_bytes = fs::read(&staged_path)
        .with_context(|| format!("failed to read {}", staged_path.display()))?;
    let output_sha256 = sha256(&staged_bytes);
    if output_path.exists() {
        let existing = fs::read(output_path)
            .with_context(|| format!("failed to read {}", output_path.display()))?;
        ensure!(
            sha256(&existing) == output_sha256 || refresh_existing,
            "deterministic label master differs from the existing output; pass --refresh-existing after reviewing the rasterization change"
        );
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(&staged_path, output_path)
        .with_context(|| format!("failed to install {}", output_path.display()))?;

    Ok(DeterministicLabelMasterReport {
        schema: 1,
        id: configuration.id.clone(),
        text: configuration.text.clone(),
        font_id: configuration.font_id.clone(),
        font_sha256: configuration.font_sha256.clone(),
        pixel_size: configuration.pixel_size,
        coverage_threshold: configuration.coverage_threshold,
        outline_radius: configuration.outline_radius,
        width: u32::try_from(raster.width)?,
        height: u32::try_from(raster.height)?,
        fill_pixels,
        outline_pixels,
        output_path: output_path.to_string_lossy().into_owned(),
        output_sha256,
        evidence_boundary: "The master is a deterministic raster of one fixed Korean label through an exact-hash font and explicit threshold, outline, and colors. Native texture placement, palette reduction, source removal, runtime display, and human approval are separate checks.",
    })
}
