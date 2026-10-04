use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use fontdue::Font;
use serde::Deserialize;

use crate::bitmap_font::{GlyphPixel, RasterizedLine, load_font, rasterize_line};
use crate::indexed_texture::sha256;

pub(super) const TEXTURE_ROW_HEIGHT_PIXELS: usize = 24;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoryDialogueFontConfiguration {
    pub(super) schema: u32,
    pub(super) id: String,
    pub(super) expected_filename: String,
    pub(super) sha256: String,
    pub(super) license: String,
    pub(super) upstream_repository: String,
    #[serde(default)]
    pub(super) glyph_aliases: BTreeMap<char, char>,
    pub(super) rasterization: StoryDialogueRasterizationConfiguration,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoryDialogueRasterizationConfiguration {
    pub(super) pixel_size: f32,
    pub(super) coverage_threshold: u8,
    pub(super) outline_radius: usize,
}

pub(super) struct LoadedStoryDialogueFont {
    pub(super) config: StoryDialogueFontConfiguration,
    font: Font,
}

pub(super) type StoryDialogueGlyphPixel = GlyphPixel;
pub(super) type RasterizedStoryDialogueLine = RasterizedLine;

pub(super) fn load_story_dialogue_font(
    config_path: &Path,
    font_path: &Path,
) -> Result<LoadedStoryDialogueFont> {
    let config: StoryDialogueFontConfiguration = serde_json::from_slice(
        &fs::read(config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", config_path.display()))?;
    ensure!(config.schema == 1, "unsupported story dialogue font schema");
    ensure!(
        font_path.file_name().and_then(|name| name.to_str()) == Some(&config.expected_filename),
        "story dialogue font filename differs from its fixed configuration"
    );
    ensure!(
        config.rasterization.pixel_size.is_finite()
            && config.rasterization.pixel_size > 0.0
            && config.rasterization.pixel_size <= TEXTURE_ROW_HEIGHT_PIXELS as f32,
        "story dialogue font size does not fit the 24-pixel row"
    );
    ensure!(
        config.rasterization.coverage_threshold > 0
            && config.rasterization.outline_radius <= 2
            && !config.id.is_empty()
            && !config.license.is_empty()
            && !config.upstream_repository.is_empty(),
        "story dialogue font provenance or rasterization is incomplete"
    );
    let bytes =
        fs::read(font_path).with_context(|| format!("failed to read {}", font_path.display()))?;
    ensure!(
        sha256(&bytes) == config.sha256,
        "story dialogue font SHA-256 differs from its fixed configuration"
    );
    let font = load_font(bytes, "story dialogue font")?;
    for (&source, &glyph) in &config.glyph_aliases {
        ensure!(
            font.lookup_glyph_index(source) == 0 && font.lookup_glyph_index(glyph) != 0,
            "story glyph alias must map an absent character to an available glyph: {source:?} -> {glyph:?}"
        );
    }
    Ok(LoadedStoryDialogueFont { config, font })
}

pub(super) fn rasterize_story_dialogue_line(
    loaded: &LoadedStoryDialogueFont,
    text: &str,
) -> Result<RasterizedStoryDialogueLine> {
    let rasterization = loaded.config.rasterization;
    // Preserve the transcription; only select the explicitly configured font glyph.
    let glyph_text: String = text
        .chars()
        .map(|character| {
            loaded
                .config
                .glyph_aliases
                .get(&character)
                .copied()
                .unwrap_or(character)
        })
        .collect();
    rasterize_line(
        &loaded.font,
        &glyph_text,
        rasterization.pixel_size,
        rasterization.coverage_threshold,
        rasterization.outline_radius,
    )
}
