use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use fontdue::Font;

use super::model::StaticUiFontConfiguration;
use crate::bitmap_font::load_font;
use crate::indexed_texture::sha256;

pub(crate) struct LoadedStaticUiFont {
    pub(crate) config: StaticUiFontConfiguration,
    pub(crate) font: Font,
}

pub(crate) fn load_static_ui_font(
    config_path: &Path,
    font_path: &Path,
) -> Result<LoadedStaticUiFont> {
    let config: StaticUiFontConfiguration = serde_json::from_slice(
        &fs::read(config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", config_path.display()))?;
    ensure!(config.schema == 1, "unsupported static UI font schema");
    ensure!(
        font_path.file_name().and_then(|name| name.to_str()) == Some(&config.expected_filename),
        "static UI font filename differs from its fixed configuration"
    );
    ensure!(
        config.rasterization.minimum_pixel_size > 0
            && config.rasterization.minimum_pixel_size <= config.rasterization.maximum_pixel_size
            && config.rasterization.maximum_pixel_size <= 96
            && config.rasterization.coverage_threshold > 0
            && config.rasterization.outline_radius <= 2
            && !config.id.is_empty()
            && !config.license.is_empty()
            && !config.upstream_repository.is_empty(),
        "static UI font provenance or rasterization is incomplete"
    );
    let bytes =
        fs::read(font_path).with_context(|| format!("failed to read {}", font_path.display()))?;
    ensure!(
        sha256(&bytes) == config.sha256,
        "static UI font SHA-256 differs from its fixed configuration"
    );
    let font = load_font(bytes, "static UI font")?;
    Ok(LoadedStaticUiFont { config, font })
}
