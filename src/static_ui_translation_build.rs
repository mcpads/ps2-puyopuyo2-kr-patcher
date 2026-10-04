use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};
use serde::Serialize;

use crate::cpx::{CPX_BLOCK_ALIGNMENT, decode_cpx_block, encode_cpx_block_to_fit};
use crate::cpx_patch::{FileRecordStoredLengthWrite, patch_file_record_stored_lengths};
use crate::disc_patch::{
    ComponentBuildMode, ComponentBuildOutput, IsoFileCandidate, SectorPatch, plan_sector_patches,
    verify_iso_readback, write_mode2_cue, write_patched_bin,
};
use crate::executable::read_executable_symbol;
use crate::graphics_translation::asset_naming::linked_texture_surface_file_stem;
use crate::graphics_translation::raster::{read_rgba_surface, verify_hashed_asset};
use crate::graphics_translation::static_regions::{StaticRegionCatalog, StaticSurfaceDeclaration};
use crate::graphics_translation::static_ui::{
    StaticUiIndexCompositionInputs, compose_static_ui_indices,
};
use crate::graphics_translation::{
    GraphicsTextRegion, GraphicsTranslationBundle, GraphicsTranslationEntry,
    GraphicsTranslationStatus, GraphicsTreatmentProfile,
};
use crate::indexed_texture::{
    FileRecord, colorize_psmt8, parse_file_records, read_decoded_image, read_palette, sha256,
    swizzle_psmt8_uploaded_as_psmct32, unswizzle_psmt8_uploaded_as_psmct32, write_rgba_png,
};
use crate::source::{DiscInventory, find_inventory_file};
use crate::texture_preview_corpus::{TexturePreview, TexturePreviewCorpus};

const EXECUTABLE_ISO_PATH: &str = "/SLPM_624.00;1";
const IMAGE_D_ISO_PATH: &str = "/IMAGE_D.BIN;1";
const IMAGE_G_ISO_PATH: &str = "/IMAGE_G.BIN;1";
const IMAGE_MD_ISO_PATH: &str = "/IMAGE_MD.BIN;1";

pub struct StaticUiTranslationBuildInputs<'a> {
    pub inventory: &'a DiscInventory,
    pub source_bin_path: &'a Path,
    pub executable: &'a [u8],
    pub image_d: &'a [u8],
    pub image_g: &'a [u8],
    pub image_md: &'a [u8],
    pub graphics: &'a GraphicsTranslationBundle,
    pub catalog: &'a StaticRegionCatalog,
    pub previews: &'a TexturePreviewCorpus,
    pub output_bin_path: &'a Path,
    pub output_cue_path: &'a Path,
    pub preview_directory: &'a Path,
    pub scope: StaticGraphicsOverlayScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticGraphicsOverlayScope {
    FunctionalUi,
    FontAtlas,
    BuildableStaticGraphics,
}

impl StaticGraphicsOverlayScope {
    fn selects_profile(self, profile: GraphicsTreatmentProfile) -> bool {
        match self {
            Self::FunctionalUi => profile == GraphicsTreatmentProfile::FunctionalUiText,
            Self::FontAtlas => profile == GraphicsTreatmentProfile::FontAtlas,
            Self::BuildableStaticGraphics => matches!(
                profile,
                GraphicsTreatmentProfile::FunctionalUiText
                    | GraphicsTreatmentProfile::FontAtlas
                    | GraphicsTreatmentProfile::StylizedDisplayLettering
            ),
        }
    }

    fn selects_region(self, region: &GraphicsTextRegion) -> bool {
        self.selects_profile(region.treatment_profile)
            && (region.treatment_profile != GraphicsTreatmentProfile::StylizedDisplayLettering
                || region
                    .artwork_plan
                    .is_some_and(|plan| plan.placement_status.is_buildable()))
    }

    fn selected_profiles(self) -> Vec<GraphicsTreatmentProfile> {
        match self {
            Self::FunctionalUi => vec![GraphicsTreatmentProfile::FunctionalUiText],
            Self::FontAtlas => vec![GraphicsTreatmentProfile::FontAtlas],
            Self::BuildableStaticGraphics => vec![
                GraphicsTreatmentProfile::FunctionalUiText,
                GraphicsTreatmentProfile::FontAtlas,
                GraphicsTreatmentProfile::StylizedDisplayLettering,
            ],
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::FunctionalUi => "functional UI",
            Self::FontAtlas => "font atlas",
            Self::BuildableStaticGraphics => "buildable static graphics",
        }
    }

    fn write_owner(self) -> &'static str {
        match self {
            Self::FunctionalUi => "static-ui-translation-development",
            Self::FontAtlas => "font-atlas-translation-development",
            Self::BuildableStaticGraphics => "static-graphics-translation-development",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StaticUiTranslationBuildReport {
    pub schema: u32,
    pub scope: StaticGraphicsOverlayScope,
    pub selected_treatment_profiles: Vec<GraphicsTreatmentProfile>,
    pub status: &'static str,
    pub release_candidate_eligible: bool,
    pub source_bin_sha256: String,
    pub output_bin_path: String,
    pub output_bin_byte_length: u64,
    pub output_bin_sha256: String,
    pub output_cue_path: String,
    pub built_surfaces: usize,
    pub built_regions: usize,
    pub source_japanese_pixels: usize,
    pub korean_overlay_pixels: usize,
    pub changed_palette_indices: usize,
    pub protected_pixels: usize,
    pub changed_protected_indices: usize,
    pub protected_english_pixels: usize,
    pub changed_protected_english_indices: usize,
    pub image_container_expected_writes: usize,
    pub executable_expected_writes: usize,
    pub raw_sector_plans: usize,
    pub raw_sector_data_writes: usize,
    pub modified_lbas: Vec<u32>,
    pub surfaces: Vec<StaticUiBuiltSurface>,
    pub verification: StaticUiBuildVerification,
}

#[derive(Clone, Debug, Serialize)]
pub struct StaticUiBuiltSurface {
    pub source_surface_id: String,
    pub container_iso_path: String,
    pub image_file_record: usize,
    pub palette_file_record: usize,
    pub width: u32,
    pub height: u32,
    pub region_count: usize,
    pub overlay_path: String,
    pub overlay_png_sha256: String,
    pub preview_path: Option<String>,
    pub preview_png_sha256: Option<String>,
    pub source_linear_indices_sha256: String,
    pub replacement_linear_indices_sha256: String,
    pub replacement_rgba_sha256: String,
    pub source_japanese_pixels: usize,
    pub korean_overlay_pixels: usize,
    pub changed_palette_indices: usize,
    pub protected_pixels: usize,
    pub changed_protected_indices: usize,
    pub protected_english_pixels: usize,
    pub changed_protected_english_indices: usize,
    pub source_cpx_byte_length: usize,
    pub decoded_trailing_byte_length: usize,
    pub replacement_cpx_byte_length: usize,
    pub cpx_slot_byte_length: usize,
    pub cpx_padding_byte_length: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct StaticUiBuildVerification {
    pub replacement_source: &'static str,
    pub source_identity: &'static str,
    pub palette_mapping: &'static str,
    pub protected_pixels: &'static str,
    pub cpx_roundtrip: &'static str,
    pub expected_writes: &'static str,
    pub output_iso_readback: &'static str,
    pub runtime_status: &'static str,
    pub human_review: &'static str,
}

struct ContainerWork<'a> {
    file_list_symbol: &'static str,
    source: &'a [u8],
    records: Vec<FileRecord>,
    file_list_offset: usize,
    plan: WritePlan,
    patched: Option<Vec<u8>>,
}

struct ReadbackSurface {
    container_iso_path: String,
    file_list_symbol: &'static str,
    image_file_record: usize,
    width: usize,
    height: usize,
    replacement_indices: Vec<u8>,
    decoded_trailing_bytes: Vec<u8>,
    replacement_cpx_byte_length: usize,
}

pub fn build_static_ui_translation_development(
    inputs: StaticUiTranslationBuildInputs<'_>,
) -> Result<StaticUiTranslationBuildReport> {
    match build_static_ui_translation(inputs, ComponentBuildMode::WriteArtifact)? {
        ComponentBuildOutput::Artifact(report) => Ok(report),
        ComponentBuildOutput::SectorPatches(_) => unreachable!("artifact mode returns a report"),
    }
}

pub(crate) fn plan_static_ui_translation(
    inputs: StaticUiTranslationBuildInputs<'_>,
) -> Result<Vec<SectorPatch>> {
    match build_static_ui_translation(inputs, ComponentBuildMode::ReturnSectorPatches)? {
        ComponentBuildOutput::Artifact(_) => {
            unreachable!("sector-patch mode returns planned sectors")
        }
        ComponentBuildOutput::SectorPatches(patches) => Ok(patches),
    }
}

fn build_static_ui_translation(
    inputs: StaticUiTranslationBuildInputs<'_>,
    mode: ComponentBuildMode,
) -> Result<ComponentBuildOutput<StaticUiTranslationBuildReport>> {
    validate_build_identity(&inputs)?;
    let declarations = inputs
        .catalog
        .surfaces
        .iter()
        .map(|surface| (surface.source_surface_id.as_str(), surface))
        .collect::<BTreeMap<_, _>>();
    let mut containers = BTreeMap::from([
        (
            IMAGE_D_ISO_PATH,
            load_container_work(inputs.executable, "FileList_Image", inputs.image_d)?,
        ),
        (
            IMAGE_G_ISO_PATH,
            load_container_work(inputs.executable, "FileList_Image2", inputs.image_g)?,
        ),
        (
            IMAGE_MD_ISO_PATH,
            load_container_work(inputs.executable, "FileList_Image3", inputs.image_md)?,
        ),
    ]);
    let mut stored_length_writes = Vec::new();
    let mut surfaces = Vec::new();
    let mut readbacks = Vec::new();

    for target in inputs.graphics.entries.iter().filter(|entry| {
        requires_role_overlay(
            entry,
            inputs.scope,
            declarations.contains_key(entry.source.source_surface_id.as_str()),
        )
    }) {
        validate_target(target, inputs.scope)?;
        let declaration = declarations
            .get(target.source.source_surface_id.as_str())
            .with_context(|| {
                format!(
                    "static UI build catalog has no source surface: {}",
                    target.source.source_surface_id
                )
            })?;
        ensure!(
            declaration.regions.len() == target.authoring.regions.len()
                && declaration
                    .regions
                    .iter()
                    .zip(&target.authoring.regions)
                    .all(|(declared, authored)| {
                        declared.region_id == authored.region_id
                            && declared.treatment_profile == authored.treatment_profile
                    }),
            "static UI build target differs from its complete region inventory: {}",
            target.source.source_surface_id
        );
        let selected_regions = target
            .authoring
            .regions
            .iter()
            .filter(|region| inputs.scope.selects_region(region))
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            !selected_regions.is_empty(),
            "static graphics build target has no region selected by {:?}: {}",
            inputs.scope,
            target.source.source_surface_id
        );
        let primary_preview = find_primary_preview(inputs.previews, target, declaration)?;
        let work = containers
            .get_mut(target.source.container_iso_path.as_str())
            .with_context(|| {
                format!(
                    "static UI target uses an unsupported image container: {}",
                    target.source.container_iso_path
                )
            })?;
        let image_record = *work
            .records
            .get(target.source.image_file_record)
            .context("static UI image file record is out of range")?;
        let palette_record = *work
            .records
            .get(primary_preview.palette_file_record)
            .context("static UI palette file record is out of range")?;
        ensure!(
            image_record.width == target.source.width
                && image_record.height == target.source.height
                && image_record.width > 0
                && image_record.height > 0,
            "static UI image record dimensions differ from the graphics ledger: {}",
            target.source.source_surface_id
        );
        let width = usize::try_from(image_record.width)?;
        let height = usize::try_from(image_record.height)?;
        let source_swizzled = read_decoded_image(work.source, image_record)?;
        let image_byte_length = width
            .checked_mul(height)
            .context("static UI image byte length overflow")?;
        ensure!(
            source_swizzled.len() >= image_byte_length,
            "static UI decoded CPX is shorter than its image dimensions: {}",
            target.source.source_surface_id
        );
        let (source_swizzled_image, decoded_trailing_bytes) =
            source_swizzled.split_at(image_byte_length);
        let source_linear =
            unswizzle_psmt8_uploaded_as_psmct32(source_swizzled_image, width, height)?;
        ensure!(
            sha256(&source_linear) == target.source.linear_indices_sha256,
            "static UI source indices differ from the graphics ledger: {}",
            target.source.source_surface_id
        );
        let palette = read_palette(
            work.source,
            palette_record,
            primary_preview.clear_first_alpha,
        )?;
        let source_rgba = colorize_psmt8(&source_linear, &palette);
        ensure!(
            sha256(&source_rgba) == primary_preview.rgba_sha256,
            "static UI source palette render differs from the primary preview: {}",
            target.source.source_surface_id
        );
        verify_all_declared_preview_variants(
            inputs.previews,
            target,
            declaration,
            work,
            &source_linear,
        )?;

        let overlay_asset = target
            .authoring
            .replacement_overlay
            .as_ref()
            .context("review-ready static UI target has no role overlay")?;
        verify_hashed_asset(overlay_asset)?;
        let overlay = read_rgba_surface(Path::new(&overlay_asset.path))?;
        ensure!(
            overlay.width == width && overlay.height == height,
            "static UI overlay dimensions differ from its source: {}",
            target.source.source_surface_id
        );
        let composition = compose_static_ui_indices(StaticUiIndexCompositionInputs {
            source_indices: &source_linear,
            palette: &palette,
            width,
            height,
            regions: &selected_regions,
            protected_english_mask: target.authoring.protected_english_mask.as_ref(),
            overlay: &overlay.pixels,
        })?;
        let replacement_rgba = colorize_psmt8(&composition.replacement_indices, &palette);
        let stem = linked_texture_surface_file_stem(&target.source.source_surface_id)?;
        let preview_path = inputs
            .preview_directory
            .join(format!("{stem}.inserted.png"));
        let (preview_path, preview_png_sha256) = if mode == ComponentBuildMode::WriteArtifact {
            write_rgba_png(
                &preview_path,
                image_record.width,
                image_record.height,
                &replacement_rgba,
            )?;
            let preview_png_sha256 = sha256(
                &fs::read(&preview_path)
                    .with_context(|| format!("failed to hash {}", preview_path.display()))?,
            );
            (
                Some(preview_path.to_string_lossy().into_owned()),
                Some(preview_png_sha256),
            )
        } else {
            (None, None)
        };

        let mut replacement_decoded =
            swizzle_psmt8_uploaded_as_psmct32(&composition.replacement_indices, width, height)?;
        replacement_decoded.extend_from_slice(decoded_trailing_bytes);
        let slot_length = usize::try_from(image_record.sector_count)?
            .checked_mul(CPX_BLOCK_ALIGNMENT)
            .context("static UI CPX slot length overflow")?;
        let replacement_cpx = encode_cpx_block_to_fit(
            &replacement_decoded,
            u32::try_from(image_byte_length)?,
            slot_length,
        )?;
        let decoded = decode_cpx_block(&replacement_cpx)?;
        ensure!(
            decoded.encoded_byte_length == replacement_cpx.len()
                && decoded.header_value == u32::try_from(image_byte_length)?
                && decoded.bytes == replacement_decoded,
            "static UI CPX round trip differs: {}",
            target.source.source_surface_id
        );
        let slot_offset = usize::try_from(image_record.sector_offset)?
            .checked_mul(CPX_BLOCK_ALIGNMENT)
            .context("static UI CPX slot offset overflow")?;

        ensure!(
            replacement_cpx.len() <= slot_length,
            "static UI replacement CPX is {} bytes but {} has only a {slot_length}-byte slot",
            replacement_cpx.len(),
            target.source.source_surface_id
        );
        let source_cpx_byte_length = usize::try_from(image_record.stored_byte_length)? + 1;
        let mut replacement_slot = replacement_cpx.clone();
        replacement_slot.resize(slot_length, 0);
        let plan = std::mem::replace(&mut work.plan, WritePlan::new());
        work.plan = plan
            .region(ImageRegion {
                id: format!("{stem}-cpx-slot"),
                range: slot_offset..slot_offset + slot_length,
                kind: RegionKind::Data,
                reason: format!(
                    "consumer-linked {} CPX slot for {}",
                    inputs.scope.label(),
                    target.source.source_surface_id
                ),
            })
            .write(ExpectedWrite {
                id: format!("replace-{stem}-cpx-slot"),
                owner: inputs.scope.write_owner().to_string(),
                purpose: format!(
                    "insert bounded pre-human-review Korean {} role overlays",
                    inputs.scope.label()
                ),
                offset: slot_offset,
                expected_original: work.source[slot_offset..slot_offset + slot_length].to_vec(),
                replacement: replacement_slot,
                intent: WriteIntent::Data,
            });
        stored_length_writes.push(FileRecordStoredLengthWrite {
            file_list_offset: work.file_list_offset,
            record_index: target.source.image_file_record,
            expected_stored_length: image_record.stored_byte_length,
            replacement_cpx_length: replacement_cpx.len(),
            region_id: format!("{stem}-cpx-stored-length"),
            region_reason: format!(
                "{} record {} CPX stored-length field",
                work.file_list_symbol, target.source.image_file_record
            ),
            write_id: format!("update-{stem}-cpx-stored-length"),
            owner: inputs.scope.write_owner().to_string(),
            purpose: format!(
                "make the source loader consume the complete {} replacement CPX stream",
                inputs.scope.label()
            ),
        });
        surfaces.push(StaticUiBuiltSurface {
            source_surface_id: target.source.source_surface_id.clone(),
            container_iso_path: target.source.container_iso_path.clone(),
            image_file_record: target.source.image_file_record,
            palette_file_record: primary_preview.palette_file_record,
            width: target.source.width,
            height: target.source.height,
            region_count: selected_regions.len(),
            overlay_path: overlay_asset.path.clone(),
            overlay_png_sha256: overlay_asset.sha256.clone(),
            preview_path,
            preview_png_sha256,
            source_linear_indices_sha256: sha256(&source_linear),
            replacement_linear_indices_sha256: sha256(&composition.replacement_indices),
            replacement_rgba_sha256: sha256(&replacement_rgba),
            source_japanese_pixels: composition.source_japanese_pixels,
            korean_overlay_pixels: composition.korean_overlay_pixels,
            changed_palette_indices: composition.changed_indices,
            protected_pixels: composition.protected_pixels,
            changed_protected_indices: composition.changed_protected_indices,
            protected_english_pixels: composition.protected_english_pixels,
            changed_protected_english_indices: composition.changed_protected_english_indices,
            source_cpx_byte_length,
            decoded_trailing_byte_length: decoded_trailing_bytes.len(),
            replacement_cpx_byte_length: replacement_cpx.len(),
            cpx_slot_byte_length: slot_length,
            cpx_padding_byte_length: slot_length - replacement_cpx.len(),
        });
        readbacks.push(ReadbackSurface {
            container_iso_path: target.source.container_iso_path.clone(),
            file_list_symbol: work.file_list_symbol,
            image_file_record: target.source.image_file_record,
            width,
            height,
            replacement_indices: composition.replacement_indices,
            decoded_trailing_bytes: decoded_trailing_bytes.to_vec(),
            replacement_cpx_byte_length: replacement_cpx.len(),
        });
    }

    let expected_surface_count = inputs
        .graphics
        .entries
        .iter()
        .filter(|entry| {
            requires_role_overlay(
                entry,
                inputs.scope,
                declarations.contains_key(entry.source.source_surface_id.as_str()),
            )
        })
        .count();
    let expected_region_count = inputs
        .graphics
        .entries
        .iter()
        .filter(|entry| {
            requires_role_overlay(
                entry,
                inputs.scope,
                declarations.contains_key(entry.source.source_surface_id.as_str()),
            )
        })
        .flat_map(|entry| &entry.authoring.regions)
        .filter(|region| inputs.scope.selects_region(region))
        .count();
    ensure!(
        surfaces.len() == expected_surface_count
            && surfaces
                .iter()
                .map(|surface| surface.region_count)
                .sum::<usize>()
                == expected_region_count,
        "static graphics development build did not consume every region selected by {:?}",
        inputs.scope
    );
    let image_container_expected_writes = containers
        .values()
        .map(|container| container.plan.writes.len())
        .sum();
    for container in containers.values_mut() {
        let plan = std::mem::replace(&mut container.plan, WritePlan::new());
        container.patched = Some(plan.apply(container.source, None)?);
    }
    let (patched_executable, executable_expected_writes) =
        patch_file_record_stored_lengths(inputs.executable, &stored_length_writes)?;

    for expected in &readbacks {
        let container = containers
            .get(expected.container_iso_path.as_str())
            .context("static UI readback container disappeared")?;
        let patched_container = container
            .patched
            .as_deref()
            .context("static UI container was not patched")?;
        let file_list = read_executable_symbol(&patched_executable, expected.file_list_symbol)?;
        let records = parse_file_records(&file_list.bytes)?;
        let record = *records
            .get(expected.image_file_record)
            .context("static UI readback image record is out of range")?;
        ensure!(
            usize::try_from(record.stored_byte_length)? + 1 == expected.replacement_cpx_byte_length,
            "static UI executable readback stored length differs"
        );
        let decoded = read_decoded_image(patched_container, record)?;
        let image_byte_length = expected
            .width
            .checked_mul(expected.height)
            .context("static UI readback image byte length overflow")?;
        ensure!(
            decoded.len() == image_byte_length + expected.decoded_trailing_bytes.len()
                && decoded[image_byte_length..] == expected.decoded_trailing_bytes,
            "static UI patched container trailing-byte readback differs"
        );
        let linear = unswizzle_psmt8_uploaded_as_psmct32(
            &decoded[..image_byte_length],
            expected.width,
            expected.height,
        )?;
        ensure!(
            linear == expected.replacement_indices,
            "static UI patched container readback differs"
        );
    }

    let file_candidates = [
        (EXECUTABLE_ISO_PATH, patched_executable.clone()),
        (
            IMAGE_D_ISO_PATH,
            containers[IMAGE_D_ISO_PATH]
                .patched
                .clone()
                .context("IMAGE_D was not patched")?,
        ),
        (
            IMAGE_G_ISO_PATH,
            containers[IMAGE_G_ISO_PATH]
                .patched
                .clone()
                .context("IMAGE_G was not patched")?,
        ),
        (
            IMAGE_MD_ISO_PATH,
            containers[IMAGE_MD_ISO_PATH]
                .patched
                .clone()
                .context("IMAGE_MD was not patched")?,
        ),
    ]
    .into_iter()
    .map(|(iso_path, replacement)| {
        let entry = find_inventory_file(inputs.inventory, iso_path)?;
        let original = match iso_path {
            EXECUTABLE_ISO_PATH => inputs.executable,
            IMAGE_D_ISO_PATH => inputs.image_d,
            IMAGE_G_ISO_PATH => inputs.image_g,
            IMAGE_MD_ISO_PATH => inputs.image_md,
            _ => unreachable!(),
        };
        Ok(IsoFileCandidate {
            iso_path: iso_path.to_string(),
            extent_lba: entry.extent_lba,
            original: original.to_vec(),
            replacement,
        })
    })
    .collect::<Result<Vec<_>>>()?;
    let sector_patches = plan_sector_patches(inputs.source_bin_path, &file_candidates)?;

    if mode == ComponentBuildMode::ReturnSectorPatches {
        return Ok(ComponentBuildOutput::SectorPatches(sector_patches));
    }
    let output_bin_sha256 = write_patched_bin(
        inputs.source_bin_path,
        inputs.output_bin_path,
        &sector_patches,
    )?;
    write_mode2_cue(inputs.output_cue_path, inputs.output_bin_path)?;
    verify_iso_readback(inputs.output_bin_path, &file_candidates)?;

    let built_regions = surfaces.iter().map(|surface| surface.region_count).sum();
    let source_japanese_pixels = surfaces
        .iter()
        .map(|surface| surface.source_japanese_pixels)
        .sum();
    let korean_overlay_pixels = surfaces
        .iter()
        .map(|surface| surface.korean_overlay_pixels)
        .sum();
    let changed_palette_indices = surfaces
        .iter()
        .map(|surface| surface.changed_palette_indices)
        .sum();
    let protected_pixels = surfaces
        .iter()
        .map(|surface| surface.protected_pixels)
        .sum();
    let changed_protected_indices = surfaces
        .iter()
        .map(|surface| surface.changed_protected_indices)
        .sum();
    let protected_english_pixels = surfaces
        .iter()
        .map(|surface| surface.protected_english_pixels)
        .sum();
    let changed_protected_english_indices = surfaces
        .iter()
        .map(|surface| surface.changed_protected_english_indices)
        .sum();
    let modified_lbas = sector_patches.iter().map(|patch| patch.lba).collect();
    let raw_sector_data_writes = sector_patches.iter().map(|patch| patch.data_writes).sum();
    Ok(ComponentBuildOutput::Artifact(
        StaticUiTranslationBuildReport {
            schema: 1,
            scope: inputs.scope,
            selected_treatment_profiles: inputs.scope.selected_profiles(),
            status: match inputs.scope {
                StaticGraphicsOverlayScope::FunctionalUi => {
                    "pre-human-review functional UI development image built; runtime verification pending"
                }
                StaticGraphicsOverlayScope::FontAtlas => {
                    "pre-human-review font-atlas development image built; runtime verification pending"
                }
                StaticGraphicsOverlayScope::BuildableStaticGraphics => {
                    "pre-human-review buildable static-graphics development image built; remaining artwork and runtime verification pending"
                }
            },
            release_candidate_eligible: false,
            source_bin_sha256: inputs.inventory.source.bin.sha256.clone(),
            output_bin_path: path_string(inputs.output_bin_path)?,
            output_bin_byte_length: inputs.inventory.source.bin.byte_length,
            output_bin_sha256,
            output_cue_path: path_string(inputs.output_cue_path)?,
            built_surfaces: surfaces.len(),
            built_regions,
            source_japanese_pixels,
            korean_overlay_pixels,
            changed_palette_indices,
            protected_pixels,
            changed_protected_indices,
            protected_english_pixels,
            changed_protected_english_indices,
            image_container_expected_writes,
            executable_expected_writes,
            raw_sector_plans: sector_patches.len(),
            raw_sector_data_writes,
            modified_lbas,
            surfaces,
            verification: StaticUiBuildVerification {
                replacement_source: "Every replacement is composed from a hash-bound glyph-role or exact source-color overlay, exact Japanese masks, and immutable source palette indices.",
                source_identity: "The supported BIN, executable, three image containers, file-list records, preview identities, and every source linear-index hash were revalidated.",
                palette_mapping: "Functional glyph roles reuse source-selected fill and outline indices. Letter-only stylized artwork uses the exact visible source colors selected by its Japanese mask; a declared full artwork composition instead selects visible colors from its exact consumer rectangle, applies any deterministic color-count limit, and reparses every palette variant from the same source indices.",
                protected_pixels: "Only a declared full artwork composition rectangle or the union of Japanese source masks and Korean overlay pixels may change; protected English and every pixel outside those edit areas remain exact.",
                cpx_roundtrip: "Every replacement was swizzled, CPX encoded, decoded, and compared before insertion, and each stream fits its immutable source slot.",
                expected_writes: "Every selected CPX slot and executable stored-length field was planned against immutable source bytes before raw-sector application.",
                output_iso_readback: "The rebuilt Mode2/Form1 sectors pass ISO extent readback, stored-length readback, and exact replacement index readback for every surface selected by the declared build scope.",
                runtime_status: match inputs.scope {
                    StaticGraphicsOverlayScope::FunctionalUi => {
                        "not run for this exact cumulative functional UI development image"
                    }
                    StaticGraphicsOverlayScope::FontAtlas => {
                        "not run for this exact font-atlas development image"
                    }
                    StaticGraphicsOverlayScope::BuildableStaticGraphics => {
                        "not run for this exact cumulative buildable static-graphics development image"
                    }
                },
                human_review: "pending for translation, layout, generated artwork, declared deterministic background reconstruction, and real-screen presentation",
            },
        },
    ))
}

fn validate_build_identity(inputs: &StaticUiTranslationBuildInputs<'_>) -> Result<()> {
    ensure!(
        inputs.graphics.source.source_bin_sha256 == inputs.inventory.source.bin.sha256
            && inputs.graphics.source.executable_iso_path == EXECUTABLE_ISO_PATH
            && inputs.catalog.source_bin_sha256 == inputs.inventory.source.bin.sha256
            && inputs.previews.source_bin_sha256 == inputs.inventory.source.bin.sha256,
        "static UI build inputs differ from the verified source BIN"
    );
    for (path, bytes) in [
        (EXECUTABLE_ISO_PATH, inputs.executable),
        (IMAGE_D_ISO_PATH, inputs.image_d),
        (IMAGE_G_ISO_PATH, inputs.image_g),
        (IMAGE_MD_ISO_PATH, inputs.image_md),
    ] {
        let entry = find_inventory_file(inputs.inventory, path)?;
        ensure!(
            entry.byte_length == u32::try_from(bytes.len())?
                && entry.sha256.as_deref() == Some(sha256(bytes).as_str()),
            "static UI build ISO file differs from the verified inventory: {path}"
        );
    }
    let executable_sha256 = sha256(inputs.executable);
    ensure!(
        inputs.graphics.source.executable_sha256 == executable_sha256
            && inputs.previews.executable_sha256 == executable_sha256,
        "static UI build executable identity differs across product inputs"
    );
    Ok(())
}

fn requires_role_overlay(
    target: &GraphicsTranslationEntry,
    scope: StaticGraphicsOverlayScope,
    has_catalog_declaration: bool,
) -> bool {
    has_catalog_declaration
        && target
            .authoring
            .regions
            .iter()
            .any(|region| scope.selects_region(region))
}

fn validate_target(
    target: &GraphicsTranslationEntry,
    scope: StaticGraphicsOverlayScope,
) -> Result<()> {
    let has_selected_regions = target
        .authoring
        .regions
        .iter()
        .any(|region| scope.selects_region(region));
    let has_unselected_regions = target
        .authoring
        .regions
        .iter()
        .any(|region| !scope.selects_region(region));
    ensure!(
        (target.authoring.status == GraphicsTranslationStatus::NeedsHumanReview
            || (target.authoring.status == GraphicsTranslationStatus::InProgress
                && has_unselected_regions))
            && target.authoring.replacement_image.is_none()
            && target.authoring.replacement_overlay.is_some()
            && !target.authoring.regions.is_empty()
            && has_selected_regions
            && target.authoring.human_approval.is_none()
            && target.authoring.independent_review.is_none(),
        "static graphics build target is not a bounded pre-human-review surface selected by {:?}: {}",
        scope,
        target.source.source_surface_id
    );
    Ok(())
}

fn load_container_work<'a>(
    executable: &[u8],
    file_list_symbol: &'static str,
    source: &'a [u8],
) -> Result<ContainerWork<'a>> {
    let file_list = read_executable_symbol(executable, file_list_symbol)?;
    Ok(ContainerWork {
        file_list_symbol,
        source,
        records: parse_file_records(&file_list.bytes)?,
        file_list_offset: file_list.file_offset,
        plan: WritePlan::new(),
        patched: None,
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
        "static UI build has no unique primary preview: {}",
        target.source.source_surface_id
    );
    Ok(matches[0])
}

fn verify_all_declared_preview_variants(
    previews: &TexturePreviewCorpus,
    target: &GraphicsTranslationEntry,
    declaration: &StaticSurfaceDeclaration,
    container: &ContainerWork<'_>,
    source_linear: &[u8],
) -> Result<()> {
    for expected_hash in std::iter::once(&declaration.source_png_sha256)
        .chain(&declaration.additional_source_png_sha256s)
    {
        let matches = previews
            .previews
            .iter()
            .filter(|preview| {
                preview.image_container_iso_path == target.source.container_iso_path
                    && preview.image_file_record == target.source.image_file_record
                    && preview.png_sha256 == *expected_hash
            })
            .collect::<Vec<_>>();
        ensure!(
            matches.len() == 1,
            "static UI declared preview variant is missing: {}",
            target.source.source_surface_id
        );
        let preview = matches[0];
        let palette_record = *container
            .records
            .get(preview.palette_file_record)
            .context("static UI variant palette record is out of range")?;
        let palette = read_palette(container.source, palette_record, preview.clear_first_alpha)?;
        ensure!(
            sha256(&colorize_psmt8(source_linear, &palette)) == preview.rgba_sha256,
            "static UI declared preview variant differs from the source indices and palette: {}",
            target.source.source_surface_id
        );
    }
    Ok(())
}

fn path_string(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .with_context(|| format!("path is not UTF-8: {}", path.display()))?
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics_translation::{
        GraphicsAuthoring, GraphicsTextRegion, GraphicsTranslationSource, HashedAsset, PixelBounds,
    };
    use crate::texture_language_survey::TextureLanguageClassification;

    fn stylized_target_without_overlay() -> GraphicsTranslationEntry {
        GraphicsTranslationEntry {
            source: GraphicsTranslationSource {
                source_surface_id: "linked-texture/image-g/000017".to_string(),
                container_iso_path: IMAGE_G_ISO_PATH.to_string(),
                image_file_record: 17,
                width: 512,
                height: 512,
                linear_indices_sha256: "source".to_string(),
                classification: TextureLanguageClassification::JapaneseOnly,
                requires_english_pixel_preservation: false,
                owner_tables: vec!["GameTexTbl".to_string()],
            },
            authoring: GraphicsAuthoring {
                status: GraphicsTranslationStatus::InProgress,
                semantic_asset_id: Some("battle/callouts".to_string()),
                semantic_role: Some("battle_callouts".to_string()),
                continuity_group: Some("battle_presentation".to_string()),
                design_candidates: Vec::new(),
                region_inventory_complete: true,
                protected_nontext_masks: Vec::new(),
                protected_english_mask: None,
                replacement_image: None,
                replacement_overlay: None,
                regions: vec![GraphicsTextRegion {
                    artwork_uses_overlay_alpha: false,
                    region_id: "victory-callout".to_string(),
                    semantic_role: "victory_callout".to_string(),
                    bounds: PixelBounds {
                        x: 0,
                        y: 0,
                        width: 64,
                        height: 32,
                    },
                    artwork_composition_bounds: None,
                    source_restoration_bounds: Vec::new(),
                    source_background_restoration:
                        crate::graphics_translation::GraphicsSourceBackgroundRestoration::NearestSourceIndex,
                    source_fill_color: None,
                    source_text: "やった".to_string(),
                    korean_text: Some("해냈다".to_string()),
                    treatment_profile: GraphicsTreatmentProfile::StylizedDisplayLettering,
                    artwork_plan: Some(crate::graphics_translation::GraphicsArtworkPlan {
                        method: crate::graphics_translation::GraphicsArtworkMethod::GeneratedOrHandAuthored,
                        placement_status: crate::graphics_translation::GraphicsArtworkPlacementStatus::RoughRegionBounds,
                    }),
                    glyph_demand_source: None,
                    source_pixel_mask: HashedAsset {
                        path: "mask.png".to_string(),
                        sha256: "mask".to_string(),
                    },
                    translator_notes: Vec::new(),
                }],
                translator_notes: Vec::new(),
                human_approval: None,
                independent_review: None,
            },
        }
    }

    #[test]
    fn rough_stylized_art_does_not_enter_the_buildable_scope() {
        let target = stylized_target_without_overlay();
        assert!(!requires_role_overlay(
            &target,
            StaticGraphicsOverlayScope::BuildableStaticGraphics,
            true,
        ));
    }

    #[test]
    fn consumer_aligned_stylized_art_enters_the_buildable_scope() {
        let mut target = stylized_target_without_overlay();
        target.authoring.status = GraphicsTranslationStatus::NeedsHumanReview;
        target.authoring.regions[0]
            .artwork_plan
            .as_mut()
            .unwrap()
            .placement_status =
            crate::graphics_translation::GraphicsArtworkPlacementStatus::ConsumerAligned;
        target.authoring.replacement_overlay = Some(HashedAsset {
            path: "overlay.png".to_string(),
            sha256: "overlay".to_string(),
        });

        assert!(requires_role_overlay(
            &target,
            StaticGraphicsOverlayScope::BuildableStaticGraphics,
            true,
        ));
        assert!(
            validate_target(&target, StaticGraphicsOverlayScope::BuildableStaticGraphics).is_ok()
        );
    }

    #[test]
    fn source_aligned_stylized_art_enters_the_buildable_scope() {
        let mut target = stylized_target_without_overlay();
        target.authoring.status = GraphicsTranslationStatus::NeedsHumanReview;
        target.authoring.regions[0]
            .artwork_plan
            .as_mut()
            .unwrap()
            .placement_status =
            crate::graphics_translation::GraphicsArtworkPlacementStatus::SourceAligned;
        target.authoring.replacement_overlay = Some(HashedAsset {
            path: "overlay.png".to_string(),
            sha256: "overlay".to_string(),
        });

        assert!(requires_role_overlay(
            &target,
            StaticGraphicsOverlayScope::BuildableStaticGraphics,
            true,
        ));
        assert!(
            validate_target(&target, StaticGraphicsOverlayScope::BuildableStaticGraphics).is_ok()
        );
    }

    #[test]
    fn selected_functional_surface_cannot_build_without_an_overlay() {
        let mut target = stylized_target_without_overlay();
        target.authoring.status = GraphicsTranslationStatus::NeedsHumanReview;
        target.authoring.regions[0].treatment_profile = GraphicsTreatmentProfile::FunctionalUiText;
        target.authoring.regions[0].artwork_plan = None;

        assert!(requires_role_overlay(
            &target,
            StaticGraphicsOverlayScope::BuildableStaticGraphics,
            true,
        ));
        assert!(
            validate_target(&target, StaticGraphicsOverlayScope::BuildableStaticGraphics).is_err()
        );
    }

    #[test]
    fn non_catalog_surfaces_do_not_enter_the_role_overlay_build() {
        let target = stylized_target_without_overlay();
        assert!(!requires_role_overlay(
            &target,
            StaticGraphicsOverlayScope::BuildableStaticGraphics,
            false,
        ));
    }
}
