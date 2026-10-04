use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::super::raster::{read_binary_mask, verify_hashed_asset};
use super::super::{GraphicsTextRegion, GraphicsTreatmentProfile, HashedAsset};
use super::background_restoration::restore_source_indices;
use super::model::RegionRestorationInputs;
use super::palette::{
    OVERLAY_TRANSPARENT, RegionOverlayPalette, select_composition_color_indices,
    select_source_color_indices, select_source_role_indices,
};

pub(crate) struct StaticUiIndexCompositionInputs<'a> {
    pub(crate) source_indices: &'a [u8],
    pub(crate) palette: &'a [u8],
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) regions: &'a [GraphicsTextRegion],
    pub(crate) protected_english_mask: Option<&'a HashedAsset>,
    pub(crate) overlay: &'a [u8],
}

pub(crate) struct StaticUiIndexComposition {
    pub(crate) replacement_indices: Vec<u8>,
    pub(crate) source_japanese_pixels: usize,
    pub(crate) korean_overlay_pixels: usize,
    pub(crate) changed_indices: usize,
    pub(crate) protected_pixels: usize,
    pub(crate) changed_protected_indices: usize,
    pub(crate) protected_english_pixels: usize,
    pub(crate) changed_protected_english_indices: usize,
}

pub(crate) fn compose_static_ui_indices(
    inputs: StaticUiIndexCompositionInputs<'_>,
) -> Result<StaticUiIndexComposition> {
    ensure!(
        inputs.source_indices.len() == inputs.width * inputs.height
            && inputs.overlay.len() == inputs.source_indices.len() * 4
            && inputs.palette.len() == 256 * 4,
        "static UI index composition dimensions changed"
    );
    let protected_english = if let Some(asset) = inputs.protected_english_mask {
        verify_hashed_asset(asset)?;
        read_binary_mask(
            Path::new(&asset.path),
            inputs.width,
            inputs.height,
            "static UI protected English",
        )?
    } else {
        vec![false; inputs.source_indices.len()]
    };
    let mut source_masks = Vec::with_capacity(inputs.regions.len());
    let mut combined_source = vec![false; inputs.source_indices.len()];
    for region in inputs.regions {
        ensure!(
            matches!(
                region.treatment_profile,
                GraphicsTreatmentProfile::FunctionalUiText
                    | GraphicsTreatmentProfile::StylizedDisplayLettering
                    | GraphicsTreatmentProfile::FontAtlas
            ),
            "static role-overlay index composition received an unsupported region"
        );
        verify_hashed_asset(&region.source_pixel_mask)?;
        let mask = read_binary_mask(
            Path::new(&region.source_pixel_mask.path),
            inputs.width,
            inputs.height,
            &region.region_id,
        )?;
        ensure!(
            mask.iter()
                .zip(&combined_source)
                .all(|(current, previous)| !(*current && *previous)),
            "static UI source masks overlap during index composition: {}",
            region.region_id
        );
        for (combined, selected) in combined_source.iter_mut().zip(&mask) {
            *combined |= *selected;
        }
        source_masks.push(mask);
    }
    let mut editable_pixels = combined_source.clone();
    for region in inputs.regions {
        let Some(bounds) = region.artwork_composition_bounds else {
            continue;
        };
        let left = usize::try_from(bounds.x)?;
        let top = usize::try_from(bounds.y)?;
        let right = usize::try_from(bounds.x + bounds.width)?;
        let bottom = usize::try_from(bounds.y + bounds.height)?;
        ensure!(
            right <= inputs.width && bottom <= inputs.height,
            "static UI artwork composition exceeds its source surface"
        );
        for y in top..bottom {
            for x in left..right {
                editable_pixels[y * inputs.width + x] = true;
            }
        }
    }

    let mut replacement = inputs.source_indices.to_vec();
    for (region, mask) in inputs.regions.iter().zip(&source_masks) {
        if region.source_restoration_bounds.is_empty() {
            restore_source_indices(
                &mut replacement,
                inputs.source_indices,
                inputs.palette,
                RegionRestorationInputs {
                    surface_width: inputs.width,
                    surface_height: inputs.height,
                    bounds: region.bounds,
                    source_mask: mask,
                    all_source_masks: &combined_source,
                    protected_english: &protected_english,
                },
                region.source_background_restoration,
            )?;
        } else {
            for bounds in &region.source_restoration_bounds {
                restore_source_indices(
                    &mut replacement,
                    inputs.source_indices,
                    inputs.palette,
                    RegionRestorationInputs {
                        surface_width: inputs.width,
                        surface_height: inputs.height,
                        bounds: *bounds,
                        source_mask: mask,
                        all_source_masks: &combined_source,
                        protected_english: &protected_english,
                    },
                    region.source_background_restoration,
                )?;
            }
        }
    }

    let mut claimed_overlay = vec![false; inputs.source_indices.len()];
    for (region, source_mask) in inputs.regions.iter().zip(&source_masks) {
        ensure!(
            !region.artwork_uses_overlay_alpha || region.artwork_composition_bounds.is_some(),
            "overlay alpha requires a full artwork composition"
        );
        let transparent_index = if region.artwork_uses_overlay_alpha {
            Some(
                (0..=u8::MAX)
                    .find(|index| {
                        crate::indexed_texture::colorize_psmt8(&[*index], inputs.palette)[3] == 0
                    })
                    .context("full artwork palette has no transparent index")?,
            )
        } else {
            None
        };
        let palette_mapping = match region.treatment_profile {
            GraphicsTreatmentProfile::FunctionalUiText | GraphicsTreatmentProfile::FontAtlas => {
                let (outline_index, fill_index) = select_source_role_indices(
                    inputs.source_indices,
                    inputs.palette,
                    source_mask,
                    region.source_fill_color,
                )?;
                RegionOverlayPalette::GlyphRoles {
                    outline_index,
                    fill_index,
                }
            }
            GraphicsTreatmentProfile::StylizedDisplayLettering => {
                let colors = if let Some(bounds) = region.artwork_composition_bounds {
                    select_composition_color_indices(
                        inputs.source_indices,
                        inputs.palette,
                        inputs.width,
                        inputs.height,
                        bounds,
                        None,
                    )?
                } else {
                    select_source_color_indices(inputs.source_indices, inputs.palette, source_mask)?
                };
                RegionOverlayPalette::ExactSourceColors(colors)
            }
            _ => unreachable!("selected static graphics profiles were checked above"),
        };
        let overlay_bounds = region.artwork_composition_bounds.unwrap_or(region.bounds);
        let left = usize::try_from(overlay_bounds.x)?;
        let top = usize::try_from(overlay_bounds.y)?;
        let right = usize::try_from(overlay_bounds.x + overlay_bounds.width)?;
        let bottom = usize::try_from(overlay_bounds.y + overlay_bounds.height)?;
        for y in top..bottom {
            for x in left..right {
                let pixel = y * inputs.width + x;
                let palette_index = palette_mapping
                    .palette_index(&inputs.overlay[pixel * 4..pixel * 4 + 4])
                    .with_context(|| {
                        format!(
                            "static graphics overlay contains an invalid pixel in {}/{}",
                            region.semantic_role, region.region_id
                        )
                    })?;
                let Some(palette_index) = palette_index.or(transparent_index) else {
                    continue;
                };
                ensure!(
                    !claimed_overlay[pixel],
                    "static UI overlay pixel belongs to overlapping region bounds"
                );
                ensure!(
                    !protected_english[pixel],
                    "static UI overlay changes protected English"
                );
                claimed_overlay[pixel] = true;
                replacement[pixel] = palette_index;
            }
        }
    }
    for (pixel, claimed) in claimed_overlay.iter().enumerate() {
        let rgba = &inputs.overlay[pixel * 4..pixel * 4 + 4];
        ensure!(
            rgba == OVERLAY_TRANSPARENT || *claimed,
            "static graphics overlay has a nontransparent or nonzero-alpha pixel outside every selected region"
        );
    }

    let mut result = StaticUiIndexComposition {
        replacement_indices: replacement,
        source_japanese_pixels: combined_source.iter().filter(|selected| **selected).count(),
        korean_overlay_pixels: claimed_overlay
            .iter()
            .enumerate()
            .filter(|(pixel, selected)| **selected && inputs.overlay[pixel * 4 + 3] != 0)
            .count(),
        changed_indices: 0,
        protected_pixels: 0,
        changed_protected_indices: 0,
        protected_english_pixels: 0,
        changed_protected_english_indices: 0,
    };
    for pixel in 0..inputs.source_indices.len() {
        let changed = inputs.source_indices[pixel] != result.replacement_indices[pixel];
        result.changed_indices += usize::from(changed);
        let editable = editable_pixels[pixel] || claimed_overlay[pixel];
        result.protected_pixels += usize::from(!editable);
        result.changed_protected_indices += usize::from(changed && !editable);
        result.protected_english_pixels += usize::from(protected_english[pixel]);
        result.changed_protected_english_indices +=
            usize::from(changed && protected_english[pixel]);
    }
    ensure!(
        result.changed_indices > 0
            && result.changed_protected_indices == 0
            && result.changed_protected_english_indices == 0,
        "static UI index composition changed no Japanese pixels or changed protected pixels"
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics_translation::raster::{hashed_asset, write_binary_mask};
    use crate::graphics_translation::{GraphicsSourceBackgroundRestoration, PixelBounds};

    #[test]
    fn functional_text_can_use_declared_space_beyond_source_ink_but_not_protected_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let mask_path = directory.path().join("source-mask.png");
        write_binary_mask(&mask_path, 5, 1, &[true, true, false, false, false]).unwrap();
        let region = GraphicsTextRegion {
            region_id: "staff".into(),
            semantic_role: "staff_names".into(),
            bounds: PixelBounds {
                x: 0,
                y: 0,
                width: 3,
                height: 1,
            },
            artwork_composition_bounds: Some(PixelBounds {
                x: 0,
                y: 0,
                width: 4,
                height: 1,
            }),
            artwork_uses_overlay_alpha: false,
            source_restoration_bounds: Vec::new(),
            source_background_restoration: GraphicsSourceBackgroundRestoration::NearestSourceIndex,
            source_fill_color: None,
            source_text: "名前".into(),
            korean_text: Some("이름".into()),
            treatment_profile: GraphicsTreatmentProfile::FunctionalUiText,
            artwork_plan: None,
            glyph_demand_source: None,
            source_pixel_mask: hashed_asset(&mask_path).unwrap(),
            translator_notes: Vec::new(),
        };
        let mut palette = vec![0; 1024];
        palette[4..8].copy_from_slice(&[255, 255, 255, 128]);
        palette[8..12].copy_from_slice(&[0, 0, 0, 128]);
        palette[12..16].copy_from_slice(&[255, 0, 0, 128]);
        let overlay = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0,
        ];
        let render = |protected| {
            compose_static_ui_indices(StaticUiIndexCompositionInputs {
                source_indices: &[1, 2, 0, 0, 3],
                palette: &palette,
                width: 5,
                height: 1,
                regions: std::slice::from_ref(&region),
                protected_english_mask: protected,
                overlay: &overlay,
            })
        };
        assert_eq!(render(None).unwrap().replacement_indices, [0, 0, 0, 1, 3]);
        let protected_path = directory.path().join("english.png");
        write_binary_mask(&protected_path, 5, 1, &[false, false, false, true, false]).unwrap();
        assert!(render(Some(&hashed_asset(&protected_path).unwrap())).is_err());
    }

    #[test]
    fn full_card_overlay_can_move_transparency_without_changing_atlas_neighbors() {
        let directory = tempfile::tempdir().unwrap();
        let mask_path = directory.path().join("source-mask.png");
        write_binary_mask(&mask_path, 4, 1, &[false; 4]).unwrap();
        let bounds = PixelBounds {
            x: 0,
            y: 0,
            width: 3,
            height: 1,
        };
        let mut region = GraphicsTextRegion {
            region_id: "card".into(),
            semantic_role: "mode_card".into(),
            bounds,
            artwork_composition_bounds: Some(bounds),
            artwork_uses_overlay_alpha: true,
            source_restoration_bounds: Vec::new(),
            source_background_restoration: GraphicsSourceBackgroundRestoration::NearestSourceIndex,
            source_fill_color: None,
            source_text: "練習".into(),
            korean_text: Some("연습".into()),
            treatment_profile: GraphicsTreatmentProfile::StylizedDisplayLettering,
            artwork_plan: None,
            glyph_demand_source: None,
            source_pixel_mask: hashed_asset(&mask_path).unwrap(),
            translator_notes: Vec::new(),
        };
        let mut palette = vec![0; 1024];
        palette[4..8].copy_from_slice(&[255, 0, 0, 128]);
        let source = [1, 0, 1, 1];
        let overlay = [0, 0, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 0, 0];
        let render = |region: &GraphicsTextRegion| {
            compose_static_ui_indices(StaticUiIndexCompositionInputs {
                source_indices: &source,
                palette: &palette,
                width: 4,
                height: 1,
                regions: std::slice::from_ref(region),
                protected_english_mask: None,
                overlay: &overlay,
            })
        };
        let result = render(&region).unwrap();
        assert_eq!(result.replacement_indices, [0, 1, 1, 1]);
        assert_eq!(result.changed_protected_indices, 0);
        region.artwork_uses_overlay_alpha = false;
        assert_eq!(render(&region).unwrap().replacement_indices, [1, 1, 1, 1]);
        region.artwork_uses_overlay_alpha = true;
        region.artwork_composition_bounds = None;
        assert!(
            render(&region)
                .err()
                .unwrap()
                .to_string()
                .contains("full artwork composition")
        );
    }
}
