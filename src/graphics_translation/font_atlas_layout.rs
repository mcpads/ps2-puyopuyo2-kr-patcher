use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::static_regions::StaticRegionCatalog;
use super::{GraphicsTranslationBundle, GraphicsTreatmentProfile, PixelBounds};
use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::{TextureRegistrationPair, parse_texture_registration_pairs, sha256};
use crate::texture_pattern::parse_pattern_records;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FontAtlasLayout {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_sha256: String,
    pub registration_bindings: Vec<FontAtlasRegistrationBinding>,
    pub pattern_witnesses: Vec<FontAtlasPatternWitness>,
    pub surfaces: Vec<FontAtlasSurfaceLayout>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FontAtlasRegistrationBinding {
    pub symbol: String,
    pub slot: usize,
    pub image_load_index: usize,
    pub palette_slot: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FontAtlasPatternWitness {
    pub symbol: String,
    pub record_count: usize,
    pub records: Vec<FontAtlasPatternRecordWitness>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FontAtlasPatternRecordWitness {
    pub record_index: usize,
    pub image_load_index: usize,
    pub bounds: PixelBounds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FontAtlasSurfaceLayout {
    pub source_surface_id: String,
    pub regions: Vec<FontAtlasRegionLayout>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FontAtlasRegionLayout {
    pub region_id: String,
    pub placements: Vec<FontAtlasTextPlacement>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FontAtlasTextPlacement {
    pub placement_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_alignment: Option<super::static_ui::StaticUiLineAlignment>,
    pub text_line_index: usize,
    pub text_slice: FontAtlasTextSlice,
    pub bounds: PixelBounds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FontAtlasTextSlice {
    Whole,
    BeforeToken { token: String },
    AfterToken { token: String },
}

impl FontAtlasTextSlice {
    pub fn select<'a>(&self, line: &'a str, placement_id: &str) -> Result<&'a str> {
        let selected = match self {
            Self::Whole => line,
            Self::BeforeToken { token } => {
                line.split_once(token)
                    .with_context(|| {
                        format!("font-atlas placement {placement_id} has no token {token:?}")
                    })?
                    .0
            }
            Self::AfterToken { token } => {
                line.split_once(token)
                    .with_context(|| {
                        format!("font-atlas placement {placement_id} has no token {token:?}")
                    })?
                    .1
            }
        }
        .trim();
        ensure!(
            !selected.is_empty(),
            "font-atlas placement {placement_id} selects empty Korean text"
        );
        Ok(selected)
    }
}

pub fn validate_font_atlas_layout(
    layout: &FontAtlasLayout,
    graphics: &GraphicsTranslationBundle,
    catalog: &StaticRegionCatalog,
    executable: &[u8],
) -> Result<()> {
    ensure!(layout.schema == 1, "unsupported font-atlas layout schema");
    ensure!(
        layout.source_bin_sha256 == graphics.source.source_bin_sha256
            && layout.executable_sha256 == graphics.source.executable_sha256
            && sha256(executable) == layout.executable_sha256,
        "font-atlas layout differs from the exact graphics source"
    );
    validate_surface_layouts(layout, graphics, catalog)?;
    validate_registration_bindings(layout, executable)?;
    validate_pattern_witnesses(layout, executable)?;
    Ok(())
}

fn validate_surface_layouts(
    layout: &FontAtlasLayout,
    graphics: &GraphicsTranslationBundle,
    catalog: &StaticRegionCatalog,
) -> Result<()> {
    let graphics_by_source = graphics
        .entries
        .iter()
        .map(|entry| (entry.source.source_surface_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let catalog_by_source = catalog
        .surfaces
        .iter()
        .map(|surface| (surface.source_surface_id.as_str(), surface))
        .collect::<BTreeMap<_, _>>();
    let expected_surfaces = graphics
        .entries
        .iter()
        .filter(|entry| {
            entry
                .authoring
                .regions
                .iter()
                .any(|region| region.treatment_profile == GraphicsTreatmentProfile::FontAtlas)
        })
        .map(|entry| entry.source.source_surface_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_surfaces = BTreeSet::new();
    for surface in &layout.surfaces {
        ensure!(
            seen_surfaces.insert(surface.source_surface_id.as_str()),
            "duplicate font-atlas surface layout: {}",
            surface.source_surface_id
        );
        let graphics_entry = graphics_by_source
            .get(surface.source_surface_id.as_str())
            .with_context(|| {
                format!(
                    "font-atlas layout has no graphics surface: {}",
                    surface.source_surface_id
                )
            })?;
        let catalog_surface = catalog_by_source
            .get(surface.source_surface_id.as_str())
            .with_context(|| {
                format!(
                    "font-atlas layout has no static catalog surface: {}",
                    surface.source_surface_id
                )
            })?;
        let graphics_regions = graphics_entry
            .authoring
            .regions
            .iter()
            .filter(|region| region.treatment_profile == GraphicsTreatmentProfile::FontAtlas)
            .collect::<Vec<_>>();
        let catalog_regions = catalog_surface
            .regions
            .iter()
            .filter(|region| region.treatment_profile == GraphicsTreatmentProfile::FontAtlas)
            .collect::<Vec<_>>();
        ensure!(
            graphics_regions.len() == surface.regions.len()
                && catalog_regions.len() == surface.regions.len(),
            "font-atlas region layout does not cover its complete surface: {}",
            surface.source_surface_id
        );
        let mut seen_regions = BTreeSet::new();
        for region_layout in &surface.regions {
            ensure!(
                seen_regions.insert(region_layout.region_id.as_str())
                    && !region_layout.placements.is_empty(),
                "duplicate or empty font-atlas region layout: {}/{}",
                surface.source_surface_id,
                region_layout.region_id
            );
            let graphics_region = graphics_regions
                .iter()
                .find(|region| region.region_id == region_layout.region_id)
                .with_context(|| {
                    format!(
                        "font-atlas graphics region is missing: {}/{}",
                        surface.source_surface_id, region_layout.region_id
                    )
                })?;
            let catalog_region = catalog_regions
                .iter()
                .find(|region| region.region_id == region_layout.region_id)
                .with_context(|| {
                    format!(
                        "font-atlas catalog region is missing: {}/{}",
                        surface.source_surface_id, region_layout.region_id
                    )
                })?;
            ensure!(
                graphics_region.bounds == catalog_region.bounds
                    && graphics_region.korean_text == catalog_region.korean_text,
                "font-atlas graphics ledger differs from its static catalog: {}/{}",
                surface.source_surface_id,
                region_layout.region_id
            );
            let korean_text = graphics_region
                .korean_text
                .as_deref()
                .context("font-atlas region has no Korean text")?;
            let lines = korean_text.lines().collect::<Vec<_>>();
            let mut used_lines = BTreeSet::new();
            let mut seen_placements = BTreeSet::new();
            for placement in &region_layout.placements {
                ensure!(
                    seen_placements.insert(placement.placement_id.as_str())
                        && placement.bounds.width > 0
                        && placement.bounds.height > 0,
                    "duplicate or empty font-atlas placement: {}/{}",
                    surface.source_surface_id,
                    placement.placement_id
                );
                let right = placement
                    .bounds
                    .x
                    .checked_add(placement.bounds.width)
                    .context("font-atlas placement horizontal overflow")?;
                let bottom = placement
                    .bounds
                    .y
                    .checked_add(placement.bounds.height)
                    .context("font-atlas placement vertical overflow")?;
                ensure!(
                    placement.bounds.x >= graphics_region.bounds.x
                        && placement.bounds.y >= graphics_region.bounds.y
                        && right <= graphics_region.bounds.x + graphics_region.bounds.width
                        && bottom <= graphics_region.bounds.y + graphics_region.bounds.height,
                    "font-atlas placement leaves its region: {}/{}",
                    surface.source_surface_id,
                    placement.placement_id
                );
                let line = *lines.get(placement.text_line_index).with_context(|| {
                    format!(
                        "font-atlas placement line index is out of range: {}/{}",
                        surface.source_surface_id, placement.placement_id
                    )
                })?;
                placement.text_slice.select(line, &placement.placement_id)?;
                used_lines.insert(placement.text_line_index);
            }
            ensure!(
                used_lines == (0..lines.len()).collect(),
                "font-atlas placements do not cover every Korean line: {}/{}",
                surface.source_surface_id,
                region_layout.region_id
            );
        }
    }
    ensure!(
        seen_surfaces == expected_surfaces,
        "font-atlas layout does not cover the exact six-surface population"
    );
    Ok(())
}

fn validate_registration_bindings(layout: &FontAtlasLayout, executable: &[u8]) -> Result<()> {
    let file = object::File::parse(executable)
        .context("failed to parse executable for font-atlas registration validation")?;
    let mut seen = BTreeSet::new();
    let mut pairs_by_symbol = BTreeMap::new();
    for binding in &layout.registration_bindings {
        ensure!(
            seen.insert((binding.symbol.as_str(), binding.slot)),
            "duplicate font-atlas registration binding: {} slot {}",
            binding.symbol,
            binding.slot
        );
        if !pairs_by_symbol.contains_key(binding.symbol.as_str()) {
            let (_, bytes) = read_symbol_bytes(&file, &binding.symbol)?;
            pairs_by_symbol.insert(
                binding.symbol.as_str(),
                parse_texture_registration_pairs(&binding.symbol, &bytes)?,
            );
        }
        let pairs = &pairs_by_symbol[binding.symbol.as_str()];
        ensure!(
            pairs.get(binding.slot)
                == Some(&TextureRegistrationPair {
                    image_load_index: binding.image_load_index,
                    palette_slot: binding.palette_slot,
                }),
            "font-atlas registration binding changed: {} slot {}",
            binding.symbol,
            binding.slot
        );
    }
    ensure!(
        !layout.registration_bindings.is_empty(),
        "font-atlas layout has no registration bindings"
    );
    Ok(())
}

fn validate_pattern_witnesses(layout: &FontAtlasLayout, executable: &[u8]) -> Result<()> {
    let file = object::File::parse(executable)
        .context("failed to parse executable for font-atlas pattern validation")?;
    let mut seen_symbols = BTreeSet::new();
    for witness in &layout.pattern_witnesses {
        ensure!(
            seen_symbols.insert(witness.symbol.as_str()) && !witness.records.is_empty(),
            "duplicate or empty font-atlas pattern witness: {}",
            witness.symbol
        );
        let (_, bytes) = read_symbol_bytes(&file, &witness.symbol)?;
        let records = parse_pattern_records(&witness.symbol, &bytes)?;
        ensure!(
            records.len() == witness.record_count,
            "font-atlas pattern population changed: {}",
            witness.symbol
        );
        let mut seen_records = BTreeSet::new();
        for expected in &witness.records {
            ensure!(
                seen_records.insert(expected.record_index),
                "duplicate font-atlas pattern record witness: {} record {}",
                witness.symbol,
                expected.record_index
            );
            let actual = records.get(expected.record_index).with_context(|| {
                format!(
                    "font-atlas pattern record is out of range: {} record {}",
                    witness.symbol, expected.record_index
                )
            })?;
            ensure!(
                actual.image_load_index == expected.image_load_index
                    && actual.source_x == expected.bounds.x
                    && actual.source_y == expected.bounds.y
                    && actual.width == expected.bounds.width
                    && actual.height == expected.bounds.height,
                "font-atlas pattern rectangle changed: {} record {}",
                witness.symbol,
                expected.record_index
            );
        }
    }
    ensure!(
        seen_symbols == BTreeSet::from(["NazoEdit_Ptn", "NazoMenu_Ptn", "NazoStr_Ptn"]),
        "font-atlas pattern witness set changed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_slice_selects_only_the_declared_side_of_a_token() {
        assert_eq!(
            FontAtlasTextSlice::BeforeToken {
                token: "{count}".to_string()
            }
            .select("뿌요를 {count}개 지우기", "prefix")
            .unwrap(),
            "뿌요를"
        );
        assert_eq!(
            FontAtlasTextSlice::AfterToken {
                token: "{count}".to_string()
            }
            .select("뿌요를 {count}개 지우기", "suffix")
            .unwrap(),
            "개 지우기"
        );
    }
}
