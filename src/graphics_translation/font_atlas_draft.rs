use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use super::asset_naming::linked_texture_surface_file_stem;
use super::font_atlas_layout::{
    FontAtlasLayout, FontAtlasRegionLayout, FontAtlasTextPlacement, validate_font_atlas_layout,
};
use super::raster::{read_binary_mask, read_rgba_surface, verify_hashed_asset};
use super::static_regions::{StaticRegionCatalog, StaticSurfaceDeclaration};
use super::static_ui::{
    LineLayout, OVERLAY_FILL, OVERLAY_OUTLINE, OVERLAY_TRANSPARENT, RegionRestorationInputs,
    StaticUiLineAlignment, SurfaceDimensions, fit_region_lines, load_static_ui_font,
    restore_source_region, select_preview_colors,
};
use super::{
    GraphicsTextRegion, GraphicsTranslationBundle, GraphicsTranslationStatus,
    GraphicsTreatmentProfile, HashedAsset,
};
use crate::bitmap_font::GlyphPixel;
use crate::indexed_texture::{sha256, write_rgba_png};
use crate::texture_preview_corpus::{TexturePreview, TexturePreviewCorpus};

pub struct FontAtlasDraftInputs<'a> {
    pub graphics: &'a mut GraphicsTranslationBundle,
    pub catalog: &'a StaticRegionCatalog,
    pub layout: &'a FontAtlasLayout,
    pub previews: &'a TexturePreviewCorpus,
    pub executable: &'a [u8],
    pub font_config_path: &'a Path,
    pub font_path: &'a Path,
    pub overlay_directory: &'a Path,
    pub preview_directory: &'a Path,
    pub refresh_existing: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontAtlasDraftReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub font_id: String,
    pub font_sha256: String,
    pub rendered_surfaces: usize,
    pub rendered_regions: usize,
    pub rendered_placements: usize,
    pub source_japanese_pixels: usize,
    pub korean_overlay_pixels: usize,
    pub changed_preview_pixels: usize,
    pub changed_protected_pixels: usize,
    pub validated_registration_bindings: usize,
    pub validated_pattern_records: usize,
    pub surfaces: Vec<FontAtlasSurfaceDraft>,
    pub release_candidate_eligible: bool,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontAtlasSurfaceDraft {
    pub source_surface_id: String,
    pub source_png_sha256: String,
    pub overlay: HashedAsset,
    pub preview_path: String,
    pub preview_png_sha256: String,
    pub source_japanese_pixels: usize,
    pub korean_overlay_pixels: usize,
    pub changed_preview_pixels: usize,
    pub changed_protected_pixels: usize,
    pub regions: Vec<FontAtlasRegionDraft>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontAtlasRegionDraft {
    pub region_id: String,
    pub source_pixels: usize,
    pub placements: Vec<FontAtlasPlacementDraft>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontAtlasPlacementDraft {
    pub placement_id: String,
    pub text_line_index: usize,
    pub rendered_text: String,
    pub pixel_size: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub alignment: StaticUiLineAlignment,
}

struct StagedFontAtlasSurface {
    graphics_index: usize,
    overlay_temporary_path: PathBuf,
    overlay_final_path: PathBuf,
    overlay_sha256: String,
    report: FontAtlasSurfaceDraft,
}

pub fn render_font_atlas_drafts(inputs: FontAtlasDraftInputs<'_>) -> Result<FontAtlasDraftReport> {
    ensure!(
        inputs.graphics.source.source_bin_sha256 == inputs.catalog.source_bin_sha256
            && inputs.graphics.source.source_bin_sha256 == inputs.previews.source_bin_sha256,
        "font-atlas draft inputs differ from the exact graphics source"
    );
    validate_font_atlas_layout(
        inputs.layout,
        inputs.graphics,
        inputs.catalog,
        inputs.executable,
    )?;
    let loaded_font = load_static_ui_font(inputs.font_config_path, inputs.font_path)?;
    let graphics_by_source = inputs
        .graphics
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.source.source_surface_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let catalog_by_source = inputs
        .catalog
        .surfaces
        .iter()
        .map(|surface| (surface.source_surface_id.as_str(), surface))
        .collect::<BTreeMap<_, _>>();
    let staging = tempfile::tempdir().context("failed to create font-atlas staging directory")?;
    let mut staged = Vec::with_capacity(inputs.layout.surfaces.len());

    for surface_layout in &inputs.layout.surfaces {
        let graphics_index = *graphics_by_source
            .get(surface_layout.source_surface_id.as_str())
            .context("font-atlas graphics surface disappeared")?;
        let target = &inputs.graphics.entries[graphics_index];
        ensure!(
            matches!(
                target.authoring.status,
                GraphicsTranslationStatus::InProgress | GraphicsTranslationStatus::NeedsHumanReview
            ) && target.authoring.replacement_image.is_none()
                && target.authoring.human_approval.is_none()
                && target.authoring.independent_review.is_none(),
            "font-atlas target is not available for deterministic draft rendering: {}",
            surface_layout.source_surface_id
        );
        let declaration = catalog_by_source
            .get(surface_layout.source_surface_id.as_str())
            .context("font-atlas static catalog surface disappeared")?;
        let preview = find_primary_preview(inputs.previews, target, declaration)?;
        let source = read_rgba_surface(Path::new(&preview.png_path))?;
        ensure!(
            source.width == usize::try_from(target.source.width)?
                && source.height == usize::try_from(target.source.height)?,
            "font-atlas preview dimensions differ: {}",
            surface_layout.source_surface_id
        );
        let protected_english = if let Some(asset) = &target.authoring.protected_english_mask {
            verify_hashed_asset(asset)?;
            read_binary_mask(
                Path::new(&asset.path),
                source.width,
                source.height,
                &format!("{}/protected English", surface_layout.source_surface_id),
            )?
        } else {
            vec![false; source.width * source.height]
        };
        let (overlay, preview_pixels, regions) = prepare_font_atlas_surface(
            &loaded_font,
            &target.authoring.regions,
            &surface_layout.regions,
            &source.pixels,
            source.width,
            source.height,
            &protected_english,
        )?;
        let combined_source =
            load_combined_source_mask(&target.authoring.regions, source.width, source.height)?;
        let source_japanese_pixels = combined_source.iter().filter(|pixel| **pixel).count();
        let korean_overlay_pixels = overlay
            .chunks_exact(4)
            .filter(|pixel| pixel[3] != 0)
            .count();
        let mut changed_preview_pixels = 0;
        let mut changed_protected_pixels = 0;
        for pixel in 0..source.width * source.height {
            let changed =
                source.pixels[pixel * 4..pixel * 4 + 4] != preview_pixels[pixel * 4..pixel * 4 + 4];
            let overlay_visible = overlay[pixel * 4 + 3] != 0;
            changed_preview_pixels += usize::from(changed);
            changed_protected_pixels +=
                usize::from(changed && !combined_source[pixel] && !overlay_visible);
        }
        ensure!(
            changed_preview_pixels > 0 && changed_protected_pixels == 0,
            "font-atlas preview changed no Japanese pixels or changed protected pixels: {}",
            surface_layout.source_surface_id
        );

        let stem = linked_texture_surface_file_stem(&surface_layout.source_surface_id)?;
        let overlay_temporary_path = staging.path().join(format!("{stem}.roles.png"));
        write_rgba_png(
            &overlay_temporary_path,
            target.source.width,
            target.source.height,
            &overlay,
        )?;
        let overlay_sha256 = sha256(&fs::read(&overlay_temporary_path)?);
        let overlay_final_path = inputs.overlay_directory.join(format!("{stem}.roles.png"));
        let preview_path = inputs.preview_directory.join(format!("{stem}.preview.png"));
        write_rgba_png(
            &preview_path,
            target.source.width,
            target.source.height,
            &preview_pixels,
        )?;
        let preview_png_sha256 = sha256(&fs::read(&preview_path)?);
        staged.push(StagedFontAtlasSurface {
            graphics_index,
            overlay_temporary_path,
            overlay_final_path: overlay_final_path.clone(),
            overlay_sha256: overlay_sha256.clone(),
            report: FontAtlasSurfaceDraft {
                source_surface_id: surface_layout.source_surface_id.clone(),
                source_png_sha256: preview.png_sha256.clone(),
                overlay: HashedAsset {
                    path: overlay_final_path.to_string_lossy().into_owned(),
                    sha256: overlay_sha256,
                },
                preview_path: preview_path.to_string_lossy().into_owned(),
                preview_png_sha256,
                source_japanese_pixels,
                korean_overlay_pixels,
                changed_preview_pixels,
                changed_protected_pixels,
                regions,
            },
        });
    }
    install_font_atlas_overlays(inputs.graphics, &staged, inputs.refresh_existing)?;

    let surfaces = staged
        .into_iter()
        .map(|surface| surface.report)
        .collect::<Vec<_>>();
    Ok(FontAtlasDraftReport {
        schema: 1,
        source_bin_sha256: inputs.graphics.source.source_bin_sha256.clone(),
        font_id: loaded_font.config.id,
        font_sha256: loaded_font.config.sha256,
        rendered_surfaces: surfaces.len(),
        rendered_regions: surfaces.iter().map(|surface| surface.regions.len()).sum(),
        rendered_placements: surfaces
            .iter()
            .flat_map(|surface| &surface.regions)
            .map(|region| region.placements.len())
            .sum(),
        source_japanese_pixels: surfaces
            .iter()
            .map(|surface| surface.source_japanese_pixels)
            .sum(),
        korean_overlay_pixels: surfaces
            .iter()
            .map(|surface| surface.korean_overlay_pixels)
            .sum(),
        changed_preview_pixels: surfaces
            .iter()
            .map(|surface| surface.changed_preview_pixels)
            .sum(),
        changed_protected_pixels: surfaces
            .iter()
            .map(|surface| surface.changed_protected_pixels)
            .sum(),
        validated_registration_bindings: inputs.layout.registration_bindings.len(),
        validated_pattern_records: inputs
            .layout
            .pattern_witnesses
            .iter()
            .map(|witness| witness.records.len())
            .sum(),
        surfaces,
        release_candidate_eligible: false,
        evidence_boundary: "The exact six font_atlas surfaces and eleven Japanese regions are rendered through explicit consumer-oriented placement slices. Korean text remains owned by the graphics ledger; layout slices only select a line or the text before/after a declared dynamic or protected token. Nazo registration pairs and the declared NazoStr_Ptn, NazoMenu_Ptn, and NazoEdit_Ptn records are reparsed from the exact executable. Every Japanese source pixel is cleared, every Korean role pixel stays inside its authored region and outside protected English, and every other preview pixel remains exact. This does not prove unlisted direct/dynamic consumer rectangles, CPX insertion, runtime presentation of the Korean artifact, human approval, independent review, or release eligibility.",
    })
}

fn prepare_font_atlas_surface(
    loaded_font: &super::static_ui::LoadedStaticUiFont,
    authored_regions: &[GraphicsTextRegion],
    region_layouts: &[FontAtlasRegionLayout],
    source_rgba: &[u8],
    width: usize,
    height: usize,
    protected_english: &[bool],
) -> Result<(Vec<u8>, Vec<u8>, Vec<FontAtlasRegionDraft>)> {
    ensure!(
        source_rgba.len() == width * height * 4 && protected_english.len() == width * height,
        "font-atlas surface dimensions changed"
    );
    let selected_regions = authored_regions
        .iter()
        .filter(|region| region.treatment_profile == GraphicsTreatmentProfile::FontAtlas)
        .collect::<Vec<_>>();
    ensure!(
        selected_regions.len() == region_layouts.len(),
        "font-atlas authored and layout region populations differ"
    );
    let combined_source = load_combined_source_mask(authored_regions, width, height)?;
    let mut overlay = vec![0_u8; source_rgba.len()];
    let mut preview = source_rgba.to_vec();
    let mut reports = Vec::with_capacity(region_layouts.len());

    for region_layout in region_layouts {
        let authored = selected_regions
            .iter()
            .find(|region| region.region_id == region_layout.region_id)
            .context("font-atlas authored region disappeared")?;
        let source_mask = read_binary_mask(
            Path::new(&authored.source_pixel_mask.path),
            width,
            height,
            &authored.region_id,
        )?;
        ensure!(
            restore_source_region(
                &mut preview,
                source_rgba,
                RegionRestorationInputs {
                    surface_width: width,
                    surface_height: height,
                    bounds: authored.bounds,
                    source_mask: &source_mask,
                    all_source_masks: &combined_source,
                    protected_english,
                },
            )?,
            "font-atlas region has no background pixel inside its bounds: {}",
            authored.region_id
        );
        let korean_text = authored
            .korean_text
            .as_deref()
            .context("font-atlas region has no Korean text")?;
        let korean_lines = korean_text.lines().collect::<Vec<_>>();
        let (outline_color, fill_color) = select_preview_colors(source_rgba, &source_mask)?;
        let mut placement_reports = Vec::with_capacity(region_layout.placements.len());
        for placement in &region_layout.placements {
            let rendered_text = placement_text(placement, &korean_lines)?;
            let fitted = fit_region_lines(
                loaded_font,
                rendered_text,
                placement.bounds,
                &source_mask,
                SurfaceDimensions { width, height },
                LineLayout {
                    maximum_pixel_size: None,
                    alignment: placement.text_alignment,
                },
                &[],
            )?
            .with_context(|| {
                format!(
                    "font-atlas placement does not fit at the configured minimum size: {}",
                    placement.placement_id
                )
            })?;
            ensure!(
                fitted.lines.len() == 1,
                "font-atlas placement unexpectedly produced multiple lines"
            );
            let line = &fitted.lines[0];
            for line_y in 0..line.raster.height {
                for line_x in 0..line.raster.width {
                    let role = line.raster.pixels[line_y * line.raster.width + line_x];
                    if role == GlyphPixel::Transparent {
                        continue;
                    }
                    let x = line.x + line_x;
                    let y = line.y + line_y;
                    let pixel = y * width + x;
                    ensure!(
                        pixel < width * height && !protected_english[pixel],
                        "font-atlas placement overlaps protected English at ({x}, {y}): {}",
                        placement.placement_id,
                    );
                    ensure!(
                        x >= usize::try_from(authored.bounds.x)?
                            && x < usize::try_from(authored.bounds.x + authored.bounds.width)?
                            && y >= usize::try_from(authored.bounds.y)?
                            && y < usize::try_from(authored.bounds.y + authored.bounds.height)?,
                        "font-atlas placement leaves its authored region: {}",
                        placement.placement_id
                    );
                    let overlay_pixel = match role {
                        GlyphPixel::Transparent => unreachable!(),
                        GlyphPixel::Outline => OVERLAY_OUTLINE,
                        GlyphPixel::Fill => OVERLAY_FILL,
                    };
                    let preview_pixel = match role {
                        GlyphPixel::Transparent => unreachable!(),
                        GlyphPixel::Outline => outline_color,
                        GlyphPixel::Fill => fill_color,
                    };
                    let destination = &mut overlay[pixel * 4..pixel * 4 + 4];
                    ensure!(
                        destination == OVERLAY_TRANSPARENT,
                        "font-atlas placements overlap: {}",
                        placement.placement_id
                    );
                    destination.copy_from_slice(&overlay_pixel);
                    preview[pixel * 4..pixel * 4 + 4].copy_from_slice(&preview_pixel);
                }
            }
            placement_reports.push(FontAtlasPlacementDraft {
                placement_id: placement.placement_id.clone(),
                text_line_index: placement.text_line_index,
                rendered_text: rendered_text.to_string(),
                pixel_size: fitted.pixel_size,
                x: u32::try_from(line.x)?,
                y: u32::try_from(line.y)?,
                width: u32::try_from(line.raster.width)?,
                height: u32::try_from(line.raster.height)?,
                alignment: line.alignment,
            });
        }
        reports.push(FontAtlasRegionDraft {
            region_id: authored.region_id.clone(),
            source_pixels: source_mask.iter().filter(|pixel| **pixel).count(),
            placements: placement_reports,
        });
    }
    Ok((overlay, preview, reports))
}

fn placement_text<'a>(
    placement: &FontAtlasTextPlacement,
    korean_lines: &[&'a str],
) -> Result<&'a str> {
    let line = *korean_lines
        .get(placement.text_line_index)
        .with_context(|| {
            format!(
                "font-atlas placement line index is out of range: {}",
                placement.placement_id
            )
        })?;
    placement.text_slice.select(line, &placement.placement_id)
}

fn load_combined_source_mask(
    regions: &[GraphicsTextRegion],
    width: usize,
    height: usize,
) -> Result<Vec<bool>> {
    let mut combined = vec![false; width * height];
    for region in regions
        .iter()
        .filter(|region| region.treatment_profile == GraphicsTreatmentProfile::FontAtlas)
    {
        verify_hashed_asset(&region.source_pixel_mask)?;
        let mask = read_binary_mask(
            Path::new(&region.source_pixel_mask.path),
            width,
            height,
            &region.region_id,
        )?;
        ensure!(
            mask.iter()
                .zip(&combined)
                .all(|(current, previous)| !(*current && *previous)),
            "font-atlas source masks overlap: {}",
            region.region_id
        );
        for (destination, selected) in combined.iter_mut().zip(mask) {
            *destination |= selected;
        }
    }
    Ok(combined)
}

fn install_font_atlas_overlays(
    graphics: &mut GraphicsTranslationBundle,
    staged: &[StagedFontAtlasSurface],
    refresh_existing: bool,
) -> Result<()> {
    for surface in staged {
        let target = &graphics.entries[surface.graphics_index];
        if let Some(existing_asset) = &target.authoring.replacement_overlay {
            ensure!(
                existing_asset.path == surface.overlay_final_path.to_string_lossy(),
                "font-atlas overlay path differs from its deterministic destination: {}",
                target.source.source_surface_id
            );
            if existing_asset.sha256 == surface.overlay_sha256 {
                ensure!(
                    sha256(&fs::read(&surface.overlay_final_path)?) == surface.overlay_sha256,
                    "existing font-atlas overlay differs from the regenerated draft: {}",
                    target.source.source_surface_id
                );
            } else {
                ensure!(
                    refresh_existing,
                    "regenerated font-atlas overlay differs from the ledger; pass --refresh-existing after reviewing the deterministic layout change: {}",
                    target.source.source_surface_id
                );
            }
        } else {
            ensure!(
                refresh_existing || !surface.overlay_final_path.exists(),
                "untracked font-atlas overlay already exists: {}",
                surface.overlay_final_path.display()
            );
        }
    }
    for surface in staged {
        let target = &mut graphics.entries[surface.graphics_index];
        let needs_install = target
            .authoring
            .replacement_overlay
            .as_ref()
            .map(|asset| asset.sha256 != surface.overlay_sha256)
            .unwrap_or(true);
        if needs_install {
            if let Some(parent) = surface.overlay_final_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&surface.overlay_temporary_path, &surface.overlay_final_path)?;
            target.authoring.replacement_overlay = Some(surface.report.overlay.clone());
            target.authoring.status = GraphicsTranslationStatus::NeedsHumanReview;
        }
    }
    Ok(())
}

fn find_primary_preview<'a>(
    previews: &'a TexturePreviewCorpus,
    target: &super::GraphicsTranslationEntry,
    declaration: &StaticSurfaceDeclaration,
) -> Result<&'a TexturePreview> {
    let matches = previews
        .previews
        .iter()
        .filter(|preview| {
            preview.image_container_iso_path == target.source.container_iso_path
                && preview.image_file_record == target.source.image_file_record
                && preview.width == target.source.width
                && preview.height == target.source.height
                && preview.linear_indices_sha256 == target.source.linear_indices_sha256
                && preview.png_sha256 == declaration.source_png_sha256
        })
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "font-atlas surface has no unique primary preview: {}",
        target.source.source_surface_id
    );
    Ok(matches[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_text_keeps_dynamic_value_outside_the_baked_overlay() {
        let placement = FontAtlasTextPlacement {
            text_alignment: None,
            placement_id: "condition-count".to_string(),
            text_line_index: 0,
            text_slice: super::super::font_atlas_layout::FontAtlasTextSlice::AfterToken {
                token: "{count}".to_string(),
            },
            bounds: super::super::PixelBounds {
                x: 0,
                y: 0,
                width: 32,
                height: 32,
            },
        };
        assert_eq!(
            placement_text(&placement, &["{count}개 지우기"]).unwrap(),
            "개 지우기"
        );
    }

    #[test]
    fn placement_text_preserves_only_the_korean_sides_of_ps2() {
        let prefix = FontAtlasTextPlacement {
            text_alignment: None,
            placement_id: "save-prefix".to_string(),
            text_line_index: 0,
            text_slice: super::super::font_atlas_layout::FontAtlasTextSlice::BeforeToken {
                token: "(PS2)".to_string(),
            },
            bounds: super::super::PixelBounds {
                x: 0,
                y: 0,
                width: 32,
                height: 32,
            },
        };
        let suffix = FontAtlasTextPlacement {
            text_alignment: None,
            placement_id: "save-suffix".to_string(),
            text_line_index: 0,
            text_slice: super::super::font_atlas_layout::FontAtlasTextSlice::AfterToken {
                token: "(PS2)".to_string(),
            },
            bounds: super::super::PixelBounds {
                x: 0,
                y: 0,
                width: 32,
                height: 32,
            },
        };
        let lines = ["메모리 카드(PS2)에 저장"];
        assert_eq!(placement_text(&prefix, &lines).unwrap(), "메모리 카드");
        assert_eq!(placement_text(&suffix, &lines).unwrap(), "에 저장");
    }
}
