use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use tempfile::TempDir;

use super::super::asset_naming::linked_texture_surface_file_stem;
use super::super::raster::{read_binary_mask, read_rgba_surface, verify_hashed_asset};
use super::super::static_regions::{StaticRegionDeclaration, StaticSurfaceDeclaration};
use super::super::{
    GraphicsArtworkMethod, GraphicsTextRegion, GraphicsTranslationBundle, GraphicsTranslationEntry,
    GraphicsTranslationStatus, GraphicsTreatmentProfile, HashedAsset,
};
use super::background_restoration::restore_source_region;
use super::font::{LoadedStaticUiFont, load_static_ui_font};
use super::layout::{SurfaceDimensions, fit_region_copies, visible_unselected_pixels};
use super::model::{
    DeferredStaticTextSurface, RegionRestorationInputs, SkippedStaticUiSurface,
    StaticTextDraftInputs, StaticTextDraftReport, StaticTextDraftScope,
    StaticUiBackgroundReconstruction, StaticUiLinePlacement, StaticUiRegionDraft,
    StaticUiSurfaceDraft,
};
use super::palette::{
    OVERLAY_FILL, OVERLAY_OUTLINE, OVERLAY_TRANSPARENT, select_preview_colors_with_fill,
};
use crate::bitmap_font::GlyphPixel;
use crate::indexed_texture::{sha256, write_rgba_png};
use crate::texture_preview_corpus::{TexturePreview, TexturePreviewCorpus};

struct StagedSurface {
    graphics_index: usize,
    is_partial_surface: bool,
    overlay_temporary_path: PathBuf,
    overlay_final_path: PathBuf,
    overlay_sha256: String,
    report: StaticUiSurfaceDraft,
}

pub fn render_static_text_drafts(
    inputs: StaticTextDraftInputs<'_>,
) -> Result<StaticTextDraftReport> {
    ensure!(
        inputs.catalog.schema == 1,
        "unsupported static region catalog schema"
    );
    ensure!(
        inputs.graphics.source.source_bin_sha256 == inputs.catalog.source_bin_sha256
            && inputs.graphics.source.source_bin_sha256 == inputs.previews.source_bin_sha256,
        "static UI inputs differ from the exact graphics source"
    );
    let loaded_font = load_static_ui_font(inputs.font_config_path, inputs.font_path)?;
    let graphics_by_source = inputs
        .graphics
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.source.source_surface_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let staging = tempfile::tempdir().context("failed to create static UI staging directory")?;
    let mut selected_product_surfaces = 0;
    let mut deferred_surfaces = Vec::new();
    let mut skipped_surfaces = Vec::new();
    let mut staged = Vec::new();

    if inputs.scope == StaticTextDraftScope::DeterministicText {
        deferred_surfaces = inputs
            .graphics
            .entries
            .iter()
            .filter_map(deferred_artwork_surface)
            .collect();
    }

    for declaration in &inputs.catalog.surfaces {
        if !declaration
            .regions
            .iter()
            .any(|region| inputs.scope.selects(region.treatment_profile))
        {
            continue;
        }
        let graphics_index = *graphics_by_source
            .get(declaration.source_surface_id.as_str())
            .with_context(|| {
                format!(
                    "graphics ledger has no {} surface: {}",
                    inputs.scope.label(),
                    declaration.source_surface_id
                )
            })?;
        let target = &inputs.graphics.entries[graphics_index];
        selected_product_surfaces += 1;
        ensure!(
            matches!(
                target.authoring.status,
                GraphicsTranslationStatus::InProgress | GraphicsTranslationStatus::NeedsHumanReview
            ) && target.authoring.replacement_image.is_none()
                && target.authoring.human_approval.is_none()
                && target.authoring.independent_review.is_none(),
            "{} target is not available for deterministic draft rendering: {}",
            inputs.scope.label(),
            declaration.source_surface_id
        );
        ensure!(
            target.authoring.regions.len() == declaration.regions.len()
                && target
                    .authoring
                    .regions
                    .iter()
                    .zip(&declaration.regions)
                    .all(|(authored, declared)| {
                        authored.region_id == declared.region_id
                            && authored.treatment_profile == declared.treatment_profile
                    }),
            "{} catalog and graphics ledger region counts differ: {}",
            inputs.scope.label(),
            declaration.source_surface_id
        );
        let authored_regions = target
            .authoring
            .regions
            .iter()
            .filter(|region| inputs.scope.selects(region.treatment_profile))
            .cloned()
            .collect::<Vec<_>>();
        let declared_regions = declaration
            .regions
            .iter()
            .filter(|region| inputs.scope.selects(region.treatment_profile))
            .cloned()
            .collect::<Vec<_>>();
        let is_partial_surface = declared_regions.len() != declaration.regions.len();
        let preview = find_primary_preview(inputs.previews, target, declaration)?;
        let source = read_rgba_surface(Path::new(&preview.png_path))?;
        ensure!(
            source.width == usize::try_from(target.source.width)?
                && source.height == usize::try_from(target.source.height)?,
            "{} preview dimensions differ: {}",
            inputs.scope.label(),
            declaration.source_surface_id
        );
        let protected_english = if let Some(asset) = &target.authoring.protected_english_mask {
            verify_hashed_asset(asset)?;
            read_binary_mask(
                Path::new(&asset.path),
                source.width,
                source.height,
                &format!("{}/protected English", declaration.source_surface_id),
            )?
        } else {
            vec![false; source.width * source.height]
        };
        let preparation = prepare_surface_draft(
            &loaded_font,
            &authored_regions,
            &declared_regions,
            &source.pixels,
            SurfaceDimensions {
                width: source.width,
                height: source.height,
            },
            &protected_english,
            inputs.scope,
        )?;
        let PreparedSurface::Ready {
            overlay,
            preview_pixels,
            regions,
            source_japanese_pixels,
            korean_overlay_pixels,
            changed_preview_pixels,
            changed_protected_pixels,
        } = preparation
        else {
            let PreparedSurface::Skipped { reason } = preparation else {
                unreachable!()
            };
            skipped_surfaces.push(SkippedStaticUiSurface {
                source_surface_id: declaration.source_surface_id.clone(),
                reason,
            });
            continue;
        };

        let stem = linked_texture_surface_file_stem(&declaration.source_surface_id)?;
        let overlay_temporary_path = staging.path().join(format!("{stem}.roles.png"));
        write_rgba_png(
            &overlay_temporary_path,
            target.source.width,
            target.source.height,
            &overlay,
        )?;
        let overlay_bytes = fs::read(&overlay_temporary_path)
            .with_context(|| format!("failed to hash {}", overlay_temporary_path.display()))?;
        let overlay_sha256 = sha256(&overlay_bytes);
        let overlay_final_path = inputs.overlay_directory.join(format!("{stem}.roles.png"));
        let preview_path = inputs.preview_directory.join(format!("{stem}.preview.png"));
        write_rgba_png(
            &preview_path,
            target.source.width,
            target.source.height,
            &preview_pixels,
        )?;
        let preview_png_sha256 = sha256(
            &fs::read(&preview_path)
                .with_context(|| format!("failed to hash {}", preview_path.display()))?,
        );
        staged.push(StagedSurface {
            graphics_index,
            is_partial_surface,
            overlay_temporary_path,
            overlay_final_path: overlay_final_path.clone(),
            overlay_sha256: overlay_sha256.clone(),
            report: StaticUiSurfaceDraft {
                source_surface_id: declaration.source_surface_id.clone(),
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

    ensure!(
        selected_product_surfaces > 0,
        "static catalog has no product surfaces selected by {:?}",
        inputs.scope
    );
    ensure!(
        !staged.is_empty(),
        "no {} surface has a reconstructable source-text background and a fitting Korean draft",
        inputs.scope.label()
    );
    ensure!(
        !inputs.scope.requires_complete_render() || skipped_surfaces.is_empty(),
        "deterministic static-text rendering left unresolved surfaces: {}",
        skipped_surfaces
            .iter()
            .map(|surface| format!("{} ({})", surface.source_surface_id, surface.reason))
            .collect::<Vec<_>>()
            .join(", ")
    );
    install_staged_overlays(inputs.graphics, &staging, &staged, inputs.refresh_existing)?;
    let rendered_regions = staged
        .iter()
        .map(|surface| surface.report.regions.len())
        .sum();
    Ok(StaticTextDraftReport {
        schema: 1,
        scope: inputs.scope,
        source_bin_sha256: inputs.graphics.source.source_bin_sha256.clone(),
        font_id: loaded_font.config.id,
        font_sha256: loaded_font.config.sha256,
        selected_product_surfaces,
        deferred_surfaces,
        rendered_surfaces: staged.len(),
        rendered_regions,
        skipped_surfaces,
        surfaces: staged.into_iter().map(|surface| surface.report).collect(),
        release_candidate_eligible: false,
        evidence_boundary: match inputs.scope {
            StaticTextDraftScope::FunctionalUi => {
                "Every declared functional_ui_text region is eligible, including functional regions on a mixed surface. The role overlays contain project-authored Korean fill and outline pixels on transparency, not source artwork. Source Japanese pixels on transparent sprites are cleared to transparency; opaque sprites use deterministic nearest-pixel reconstruction restricted to the declared functional region bounds and excluding every other Japanese and protected-English pixel. Every Korean role pixel must fit its declared bounds without touching protected English or another source region, and generated previews preserve every pixel outside the selected source and overlay union. Mixed surfaces remain in_progress because stylized_display_lettering is outside this scope. Font atlases, glyph-supply surfaces, CPX insertion, runtime display, independent review, human approval, and release-candidate eligibility are not established."
            }
            StaticTextDraftScope::DeterministicText => {
                "Every product functional_ui_text region in the static catalog is rendered or the command fails. Stylized display lettering is listed separately and cannot enter this generic renderer. The tea-cup labels are explicitly excluded from the generated-or-hand-authored artwork denominator and are rendered through their own consumer-aligned deterministic raster manifests. Other-title promotional surfaces are excluded. Role overlays contain project-authored Korean fill and outline pixels on transparency, not source artwork. Source Japanese pixels are cleared through exact masks and deterministic bounded background reconstruction; every pixel outside the selected source and overlay union, including protected English, remains exact. Font atlases, glyph-supply surfaces, CPX insertion, authored-art insertion, runtime display, independent review, human approval, and release-candidate eligibility are not established."
            }
        },
    })
}

fn deferred_artwork_surface(entry: &GraphicsTranslationEntry) -> Option<DeferredStaticTextSurface> {
    let regions = entry
        .authoring
        .regions
        .iter()
        .filter(|region| {
            region.treatment_profile == GraphicsTreatmentProfile::StylizedDisplayLettering
        })
        .collect::<Vec<_>>();
    if regions.is_empty() {
        return None;
    }
    let generated_or_hand_authored = regions.iter().any(|region| {
        region
            .artwork_plan
            .is_some_and(|plan| plan.method == GraphicsArtworkMethod::GeneratedOrHandAuthored)
    });
    let deterministic_raster = regions.iter().any(|region| {
        region
            .artwork_plan
            .is_some_and(|plan| plan.method == GraphicsArtworkMethod::DeterministicRaster)
    });
    let reason = match (generated_or_hand_authored, deterministic_raster) {
        (true, false) => "consumer-aligned generated or hand-authored artwork pending",
        (false, true) => {
            "consumer-aligned deterministic placement pending; excluded from generated artwork"
        }
        (true, true) => "consumer-aligned artwork pending through mixed authoring methods",
        (false, false) => "stylized artwork plan missing",
    };
    Some(DeferredStaticTextSurface {
        source_surface_id: entry.source.source_surface_id.clone(),
        region_count: regions.len(),
        reason,
    })
}

enum PreparedSurface {
    Ready {
        overlay: Vec<u8>,
        preview_pixels: Vec<u8>,
        regions: Vec<StaticUiRegionDraft>,
        source_japanese_pixels: usize,
        korean_overlay_pixels: usize,
        changed_preview_pixels: usize,
        changed_protected_pixels: usize,
    },
    Skipped {
        reason: String,
    },
}

fn prepare_surface_draft(
    loaded_font: &LoadedStaticUiFont,
    authored_regions: &[GraphicsTextRegion],
    declarations: &[StaticRegionDeclaration],
    source_rgba: &[u8],
    dimensions: SurfaceDimensions,
    protected_english: &[bool],
    scope: StaticTextDraftScope,
) -> Result<PreparedSurface> {
    let SurfaceDimensions { width, height } = dimensions;
    ensure!(
        source_rgba.len() == width * height * 4 && protected_english.len() == width * height,
        "functional UI surface dimensions changed"
    );
    let mut overlay = vec![0_u8; source_rgba.len()];
    let mut preview = source_rgba.to_vec();
    let mut combined_source = vec![false; width * height];
    let mut region_reports = Vec::with_capacity(authored_regions.len());

    let mut source_masks = Vec::with_capacity(authored_regions.len());
    for (authored, declaration) in authored_regions.iter().zip(declarations) {
        ensure!(
            authored.region_id == declaration.region_id
                && authored.bounds == declaration.bounds
                && authored.artwork_composition_bounds == declaration.artwork_composition_bounds
                && authored.source_restoration_bounds == declaration.source_restoration_bounds
                && authored.korean_text == declaration.korean_text
                && authored.source_fill_color == declaration.source_fill_color
                && scope.selects(authored.treatment_profile),
            "{} ledger differs from its static catalog: {}",
            scope.label(),
            declaration.region_id
        );
        verify_hashed_asset(&authored.source_pixel_mask)?;
        let source_mask = read_binary_mask(
            Path::new(&authored.source_pixel_mask.path),
            width,
            height,
            &declaration.region_id,
        )?;
        ensure!(
            source_mask
                .iter()
                .zip(&combined_source)
                .all(|(current, previous)| !(*current && *previous)),
            "functional UI source regions overlap: {}",
            declaration.region_id
        );
        for (combined, selected) in combined_source.iter_mut().zip(&source_mask) {
            *combined |= *selected;
        }
        source_masks.push(source_mask);
    }

    for ((authored, declaration), source_mask) in
        authored_regions.iter().zip(declarations).zip(&source_masks)
    {
        let visible_background =
            visible_unselected_pixels(source_rgba, width, height, declaration.bounds, source_mask)?;
        let background_reconstruction = if visible_background == 0 {
            StaticUiBackgroundReconstruction::Transparent
        } else {
            StaticUiBackgroundReconstruction::NearestPixelInsideDeclaredBounds
        };
        let restored = if declaration.source_restoration_bounds.is_empty() {
            restore_source_region(
                &mut preview,
                source_rgba,
                RegionRestorationInputs {
                    surface_width: width,
                    surface_height: height,
                    bounds: declaration.bounds,
                    source_mask,
                    all_source_masks: &combined_source,
                    protected_english,
                },
            )?
        } else {
            let mut restored = true;
            for bounds in &declaration.source_restoration_bounds {
                restored &= restore_source_region(
                    &mut preview,
                    source_rgba,
                    RegionRestorationInputs {
                        surface_width: width,
                        surface_height: height,
                        bounds: *bounds,
                        source_mask,
                        all_source_masks: &combined_source,
                        protected_english,
                    },
                )?;
            }
            restored
        };
        if !restored {
            return Ok(PreparedSurface::Skipped {
                reason: format!(
                    "region {} has no background pixel inside its declared bounds after excluding Japanese and protected English pixels",
                    declaration.region_id
                ),
            });
        }
        let korean_text = authored
            .korean_text
            .as_deref()
            .context("functional UI region has no Korean text")?;
        let fitted = fit_region_copies(
            loaded_font,
            korean_text,
            declaration,
            width,
            height,
            source_mask,
        )?;
        let Some(fitted) = fitted else {
            return Ok(PreparedSurface::Skipped {
                reason: format!(
                    "region {} does not fit at the configured minimum font size",
                    declaration.region_id
                ),
            });
        };
        let (outline_color, fill_color) = select_preview_colors_with_fill(
            source_rgba,
            source_mask,
            declaration.source_fill_color,
        )?;
        let mut overlay_pixels = 0;
        for copy in &fitted {
            for line in &copy.lines {
                for line_y in 0..line.raster.height {
                    for line_x in 0..line.raster.width {
                        let role = line.raster.pixels[line_y * line.raster.width + line_x];
                        let role = if declaration.bold && role == GlyphPixel::Outline {
                            GlyphPixel::Fill
                        } else {
                            role
                        };
                        if role == GlyphPixel::Transparent {
                            continue;
                        }
                        let x = line.x + line_x;
                        let y = line.y + line_y;
                        let pixel = y * width + x;
                        if protected_english[pixel] {
                            return Ok(PreparedSurface::Skipped {
                                reason: format!(
                                    "region {} Korean draft overlaps protected English pixels",
                                    declaration.region_id
                                ),
                            });
                        }
                        if combined_source[pixel] && !source_mask[pixel] {
                            return Ok(PreparedSurface::Skipped {
                                reason: format!(
                                    "region {} Korean draft overlaps another Japanese source region",
                                    declaration.region_id
                                ),
                            });
                        }
                        let overlay_rgba = match role {
                            GlyphPixel::Transparent => unreachable!(),
                            GlyphPixel::Outline => OVERLAY_OUTLINE,
                            GlyphPixel::Fill => OVERLAY_FILL,
                        };
                        let preview_rgba = match role {
                            GlyphPixel::Transparent => unreachable!(),
                            GlyphPixel::Outline => outline_color,
                            GlyphPixel::Fill => fill_color,
                        };
                        let destination = &mut overlay[pixel * 4..pixel * 4 + 4];
                        ensure!(
                            destination == OVERLAY_TRANSPARENT,
                            "functional UI Korean regions overlap: {}",
                            declaration.region_id
                        );
                        destination.copy_from_slice(&overlay_rgba);
                        preview[pixel * 4..pixel * 4 + 4].copy_from_slice(&preview_rgba);
                        overlay_pixels += 1;
                    }
                }
            }
        }
        region_reports.push(StaticUiRegionDraft {
            region_id: declaration.region_id.clone(),
            korean_text: korean_text.to_string(),
            pixel_size: fitted
                .iter()
                .map(|copy| copy.pixel_size)
                .min()
                .expect("functional UI region has a rendered copy"),
            rendered_copies: fitted.len(),
            source_pixels: source_mask.iter().filter(|selected| **selected).count(),
            overlay_pixels,
            visible_background_pixels: visible_background,
            background_reconstruction,
            lines: fitted
                .iter()
                .flat_map(|copy| &copy.lines)
                .map(|line| StaticUiLinePlacement {
                    x: u32::try_from(line.x).expect("line x fits u32"),
                    y: u32::try_from(line.y).expect("line y fits u32"),
                    width: u32::try_from(line.raster.width).expect("line width fits u32"),
                    height: u32::try_from(line.raster.height).expect("line height fits u32"),
                    alignment: line.alignment,
                })
                .collect(),
        });
    }

    let mut changed_preview_pixels = 0;
    let mut changed_protected_pixels = 0;
    let mut korean_overlay_pixels = 0;
    for pixel in 0..width * height {
        let overlay_visible = overlay[pixel * 4 + 3] != 0;
        korean_overlay_pixels += usize::from(overlay_visible);
        let changed = source_rgba[pixel * 4..pixel * 4 + 4] != preview[pixel * 4..pixel * 4 + 4];
        changed_preview_pixels += usize::from(changed);
        if changed && !combined_source[pixel] && !overlay_visible {
            changed_protected_pixels += 1;
        }
    }
    ensure!(
        changed_preview_pixels > 0 && changed_protected_pixels == 0,
        "functional UI preview changed no Japanese pixels or changed protected pixels"
    );
    Ok(PreparedSurface::Ready {
        overlay,
        preview_pixels: preview,
        regions: region_reports,
        source_japanese_pixels: combined_source.iter().filter(|selected| **selected).count(),
        korean_overlay_pixels,
        changed_preview_pixels,
        changed_protected_pixels,
    })
}

fn find_primary_preview<'a>(
    previews: &'a TexturePreviewCorpus,
    target: &GraphicsTranslationEntry,
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
        "functional UI surface has no unique primary preview: {}",
        declaration.source_surface_id
    );
    Ok(matches[0])
}

fn install_staged_overlays(
    graphics: &mut GraphicsTranslationBundle,
    _staging: &TempDir,
    staged: &[StagedSurface],
    refresh_existing: bool,
) -> Result<()> {
    for surface in staged {
        let target = &graphics.entries[surface.graphics_index];
        if target.authoring.replacement_overlay.is_some() {
            let declared = target
                .authoring
                .replacement_overlay
                .as_ref()
                .context("review-ready functional UI target has no role overlay")?;
            ensure!(
                declared.path == surface.overlay_final_path.to_string_lossy(),
                "functional UI overlay path differs from its deterministic destination: {}",
                target.source.source_surface_id
            );
            if declared.sha256 == surface.overlay_sha256 {
                let existing = fs::read(&surface.overlay_final_path).with_context(|| {
                    format!(
                        "failed to read existing functional UI overlay {}",
                        surface.overlay_final_path.display()
                    )
                })?;
                ensure!(
                    sha256(&existing) == surface.overlay_sha256,
                    "existing functional UI overlay differs from the regenerated draft: {}",
                    target.source.source_surface_id
                );
            } else {
                ensure!(
                    refresh_existing,
                    "regenerated functional UI overlay differs from the ledger; pass --refresh-existing after reviewing the deterministic renderer change: {}",
                    target.source.source_surface_id
                );
            }
        } else {
            ensure!(
                refresh_existing || !surface.overlay_final_path.exists(),
                "untracked functional UI overlay already exists: {}",
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
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::copy(&surface.overlay_temporary_path, &surface.overlay_final_path).with_context(
                || {
                    format!(
                        "failed to install functional UI overlay {}",
                        surface.overlay_final_path.display()
                    )
                },
            )?;
            target.authoring.replacement_overlay = Some(surface.report.overlay.clone());
            if !surface.is_partial_surface {
                target.authoring.status = GraphicsTranslationStatus::NeedsHumanReview;
            }
        }
    }
    Ok(())
}
