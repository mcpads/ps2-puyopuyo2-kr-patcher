use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use super::raster::{
    RgbaSurface, hashed_asset, read_binary_mask, read_rgba_surface, selected_bounds,
    write_binary_mask,
};
use super::{
    GraphicsSourceBackgroundRestoration, GraphicsTextRegion, GraphicsTranslationBundle,
    GraphicsTranslationStatus, GraphicsTreatmentProfile, HashedAsset, PixelBounds,
};
use crate::story_dialogue_translation::StoryDialogueTranslationBundle;
use crate::texture_language_survey::TextureLanguageClassification;
use crate::texture_preview_corpus::{TexturePreview, TexturePreviewCorpus};

const STORY_SEMANTIC_ROLE: &str = "story_dialogue_baked_text";
const STORY_TEXTURE_CELL_PIXELS: usize = 24;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoryDialogueRegionInventoryReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub inventoried_surfaces: usize,
    pub skipped_surfaces: Vec<SkippedStoryDialogueRegion>,
    pub surfaces: Vec<StoryDialogueSurfaceRegion>,
    pub release_candidate_eligible: bool,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SkippedStoryDialogueRegion {
    pub story_surface_id: String,
    pub source_surface_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoryDialogueSurfaceRegion {
    pub story_surface_id: String,
    pub source_surface_id: String,
    pub consumer_table: String,
    pub source_png_sha256: String,
    pub bounds: PixelBounds,
    pub visible_source_pixels: usize,
    pub protected_english_pixels: usize,
    pub protected_pixels: usize,
    pub source_pixel_mask: HashedAsset,
    pub protected_english_mask: Option<HashedAsset>,
    pub protected_nontext_mask: HashedAsset,
}

pub fn inventory_story_dialogue_regions(
    graphics: &mut GraphicsTranslationBundle,
    story: &StoryDialogueTranslationBundle,
    previews: &TexturePreviewCorpus,
    output_directory: &Path,
) -> Result<StoryDialogueRegionInventoryReport> {
    ensure!(
        graphics.source == story.source,
        "story and graphics translation sources differ"
    );
    ensure!(
        graphics.source.source_bin_sha256 == previews.source_bin_sha256
            && graphics.source.executable_iso_path == previews.executable_iso_path
            && graphics.source.executable_sha256 == previews.executable_sha256,
        "texture previews differ from the story graphics source"
    );

    let preview_by_source = collect_story_previews(previews)?;
    let graphics_by_source = graphics
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.source.source_surface_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut surfaces = Vec::new();
    let skipped_surfaces = Vec::new();

    for story_entry in &story.entries {
        let source = &story_entry.source;
        ensure!(
            matches!(
                source.classification,
                TextureLanguageClassification::JapaneseOnly
                    | TextureLanguageClassification::JapaneseEnglishMixed
            ) && (source.classification == TextureLanguageClassification::JapaneseEnglishMixed)
                == source.requires_english_pixel_preservation,
            "story surface has an inconsistent Japanese/English classification: {}",
            source.story_surface_id
        );
        ensure!(
            !story_entry.authoring.segments.is_empty()
                && story_entry.authoring.segments.iter().all(|segment| {
                    !segment.source_text.trim().is_empty() && !segment.korean_text.trim().is_empty()
                }),
            "story surface has no complete direct draft: {}",
            source.story_surface_id
        );
        let graphics_index = *graphics_by_source
            .get(source.source_surface_id.as_str())
            .with_context(|| {
                format!(
                    "graphics ledger has no story source surface: {}",
                    source.source_surface_id
                )
            })?;
        let target = &mut graphics.entries[graphics_index];
        ensure!(
            target.source.container_iso_path == source.container_iso_path
                && target.source.image_file_record == source.image_file_record
                && target.source.width == source.width
                && target.source.height == source.height
                && target.source.linear_indices_sha256 == source.linear_indices_sha256
                && target.source.classification == source.classification
                && target.source.requires_english_pixel_preservation
                    == source.requires_english_pixel_preservation
                && target.source.owner_tables.as_slice() == [source.consumer_table.clone()],
            "story and graphics physical identities differ: {}",
            source.story_surface_id
        );
        ensure!(
            target.authoring.semantic_asset_id.as_deref() == Some(&source.story_surface_id)
                && target.authoring.semantic_role.as_deref() == Some(STORY_SEMANTIC_ROLE)
                && matches!(
                    target.authoring.status,
                    GraphicsTranslationStatus::NeedsRegionInventory
                        | GraphicsTranslationStatus::InProgress
                )
                && target.authoring.replacement_image.is_none()
                && target.authoring.replacement_overlay.is_none()
                && target.authoring.human_approval.is_none()
                && target.authoring.independent_review.is_none(),
            "story graphics target is not in a region-authoring state: {}",
            source.story_surface_id
        );

        let preview = preview_by_source
            .get(&(source.container_iso_path.as_str(), source.image_file_record))
            .with_context(|| {
                format!(
                    "story surface has no unique preview: {}",
                    source.story_surface_id
                )
            })?;
        ensure!(
            preview.width == source.width
                && preview.height == source.height
                && preview.linear_indices_sha256 == source.linear_indices_sha256
                && preview.owner_tables.as_slice() == [source.consumer_table.clone()],
            "story preview differs from its exact source: {}",
            source.story_surface_id
        );
        let surface = read_rgba_surface(Path::new(&preview.png_path))?;
        ensure!(
            surface.width == usize::try_from(source.width)?
                && surface.height == usize::try_from(source.height)?,
            "story preview dimensions differ: {}",
            source.story_surface_id
        );
        let source_text = story_entry
            .authoring
            .segments
            .iter()
            .map(|segment| segment.source_text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let visible_mask = visible_pixel_mask(&surface)?;
        let partition = partition_story_visible_pixels(
            &surface,
            &visible_mask,
            &source_text,
            source.classification,
            &source.story_surface_id,
        )?;
        let bounds = selected_bounds(
            &partition.japanese_text,
            surface.width,
            surface.height,
            &source.story_surface_id,
        )?;
        let visible_source_pixels = partition
            .japanese_text
            .iter()
            .filter(|selected| **selected)
            .count();
        let protected_english_pixels = partition
            .protected_english
            .iter()
            .filter(|selected| **selected)
            .count();
        let protected_pixels = partition
            .protected_nontext
            .iter()
            .filter(|selected| **selected)
            .count();

        let basename = format!("demo-{:02X}", source.consumer_sequence);
        let source_mask_path = output_directory.join(format!("{basename}-source.mask.png"));
        let english_mask_path =
            output_directory.join(format!("{basename}-protected-english.mask.png"));
        let protected_mask_path =
            output_directory.join(format!("{basename}-protected-nontext.mask.png"));
        if let Some(existing) = &target.authoring.protected_english_mask {
            ensure!(
                source.requires_english_pixel_preservation,
                "Japanese-only story surface declares an English mask: {}",
                source.story_surface_id
            );
            let existing_mask = read_binary_mask(
                Path::new(&existing.path),
                surface.width,
                surface.height,
                &source.story_surface_id,
            )?;
            ensure!(
                hashed_asset(Path::new(&existing.path))? == *existing
                    && existing_mask == partition.protected_english,
                "reviewed story English mask differs from its exact visible Latin cells: {}",
                source.story_surface_id
            );
        }
        write_binary_mask(
            &source_mask_path,
            surface.width,
            surface.height,
            &partition.japanese_text,
        )?;
        write_binary_mask(
            &protected_mask_path,
            surface.width,
            surface.height,
            &partition.protected_nontext,
        )?;
        let source_pixel_mask = hashed_asset(&source_mask_path)?;
        let protected_nontext_mask = hashed_asset(&protected_mask_path)?;
        let protected_english_mask = if source.requires_english_pixel_preservation {
            write_binary_mask(
                &english_mask_path,
                surface.width,
                surface.height,
                &partition.protected_english,
            )?;
            Some(hashed_asset(&english_mask_path)?)
        } else {
            None
        };

        let mut region_notes = vec![
            "발화별 문장·화자·검토 상태는 분리형 스토리 원장이 관리한다. 이 region은 물리 surface의 일본어 픽셀 모집단과 한국어 대체 이미지 경계만 소유한다."
                .to_string(),
        ];
        if source.requires_english_pixel_preservation {
            region_notes.push(
                "원문 전사의 전각 Latin 24x24 cells에서 exact visible pixels만 영어 보호 mask로 분리하며 한국어 합성은 그 고정 cells를 다시 그리지 않는다."
                    .to_string(),
            );
        }
        target.authoring.status = GraphicsTranslationStatus::InProgress;
        target.authoring.region_inventory_complete = true;
        target.authoring.protected_nontext_masks = vec![protected_nontext_mask.clone()];
        target.authoring.protected_english_mask = protected_english_mask.clone();
        target.authoring.regions = vec![GraphicsTextRegion {
            artwork_uses_overlay_alpha: false,
            region_id: "dialogue-surface".to_string(),
            semantic_role: "story_dialogue_text".to_string(),
            bounds,
            artwork_composition_bounds: None,
            source_restoration_bounds: Vec::new(),
            source_background_restoration: GraphicsSourceBackgroundRestoration::NearestSourceIndex,
            source_fill_color: None,
            source_text,
            korean_text: None,
            treatment_profile: GraphicsTreatmentProfile::CutsceneBakedText,
            artwork_plan: None,
            glyph_demand_source: None,
            source_pixel_mask: source_pixel_mask.clone(),
            translator_notes: region_notes,
        }];
        surfaces.push(StoryDialogueSurfaceRegion {
            story_surface_id: source.story_surface_id.clone(),
            source_surface_id: source.source_surface_id.clone(),
            consumer_table: source.consumer_table.clone(),
            source_png_sha256: preview.png_sha256.clone(),
            bounds,
            visible_source_pixels,
            protected_english_pixels,
            protected_pixels,
            source_pixel_mask,
            protected_english_mask,
            protected_nontext_mask,
        });
    }

    Ok(StoryDialogueRegionInventoryReport {
        schema: 1,
        source_bin_sha256: graphics.source.source_bin_sha256.clone(),
        inventoried_surfaces: surfaces.len(),
        skipped_surfaces,
        surfaces,
        release_candidate_eligible: false,
        evidence_boundary: "Each inventoried Japanese-bearing story surface is reproduced by the exact-source preview corpus. Japanese source pixels, reviewed fixed-cell English visible pixels, and non-text pixels form a non-overlapping full-surface partition. Mixed surfaces without a fixed-cell English transcription fail instead of clearing the English pixels. This inventory does not provide Korean replacement artwork, insertion, runtime display, independent review, human approval, or release-candidate eligibility.",
    })
}

fn collect_story_previews(
    previews: &TexturePreviewCorpus,
) -> Result<BTreeMap<(&str, usize), &TexturePreview>> {
    let mut selected = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for preview in &previews.previews {
        let key = (
            preview.image_container_iso_path.as_str(),
            preview.image_file_record,
        );
        if selected.insert(key, preview).is_some() {
            duplicates.insert(key);
        }
    }
    for key in duplicates {
        selected.remove(&key);
    }
    Ok(selected)
}

fn visible_pixel_mask(surface: &RgbaSurface) -> Result<Vec<bool>> {
    ensure!(
        surface.pixels.len() == surface.width * surface.height * 4,
        "story surface RGBA length differs from its dimensions"
    );
    let mask = surface
        .pixels
        .chunks_exact(4)
        .map(|pixel| pixel[3] != 0)
        .collect::<Vec<_>>();
    let selected = mask.iter().filter(|value| **value).count();
    ensure!(
        selected > 0 && selected < mask.len(),
        "story surface visible-pixel mask is empty or covers the full surface"
    );
    Ok(mask)
}

struct StoryVisiblePixelPartition {
    japanese_text: Vec<bool>,
    protected_english: Vec<bool>,
    protected_nontext: Vec<bool>,
}

fn partition_story_visible_pixels(
    surface: &RgbaSurface,
    visible_mask: &[bool],
    source_text: &str,
    classification: TextureLanguageClassification,
    context: &str,
) -> Result<StoryVisiblePixelPartition> {
    ensure!(
        visible_mask.len() == surface.width * surface.height,
        "story visible mask dimensions differ: {context}"
    );
    let source_rows = source_text.split('\n').collect::<Vec<_>>();
    let mut protected_english = vec![false; visible_mask.len()];
    let mut protected_cells = 0_usize;
    match classification {
        TextureLanguageClassification::JapaneseOnly => ensure!(
            source_rows
                .iter()
                .flat_map(|row| row.chars())
                .all(|character| !is_fixed_cell_english(character)),
            "Japanese-only story surface contains a fixed-cell English transcription: {context}"
        ),
        TextureLanguageClassification::JapaneseEnglishMixed => {
            let visible_rows = (0..surface.height.div_ceil(STORY_TEXTURE_CELL_PIXELS))
                .filter(|texture_row| {
                    let top = texture_row * STORY_TEXTURE_CELL_PIXELS;
                    let bottom = (top + STORY_TEXTURE_CELL_PIXELS).min(surface.height);
                    (top..bottom).any(|y| {
                        visible_mask[y * surface.width..(y + 1) * surface.width]
                            .iter()
                            .any(|selected| *selected)
                    })
                })
                .collect::<Vec<_>>();
            ensure!(
                source_rows.len() == visible_rows.len(),
                "story transcription rows differ from exact visible texture rows: {context}"
            );
            for (source_row, texture_row) in source_rows.iter().zip(visible_rows) {
                for (cell, character) in source_row.chars().enumerate() {
                    if !is_fixed_cell_english(character) {
                        continue;
                    }
                    let left = cell
                        .checked_mul(STORY_TEXTURE_CELL_PIXELS)
                        .context("story English cell offset overflow")?;
                    let right = left
                        .checked_add(STORY_TEXTURE_CELL_PIXELS)
                        .context("story English cell width overflow")?;
                    let top = texture_row
                        .checked_mul(STORY_TEXTURE_CELL_PIXELS)
                        .context("story English row offset overflow")?;
                    let bottom = top
                        .checked_add(STORY_TEXTURE_CELL_PIXELS)
                        .context("story English row height overflow")?;
                    ensure!(
                        right <= surface.width && bottom <= surface.height,
                        "story English cell falls outside its source texture: {context}"
                    );
                    let mut cell_pixels = 0_usize;
                    for y in top..bottom {
                        for x in left..right {
                            let index = y * surface.width + x;
                            if visible_mask[index] {
                                protected_english[index] = true;
                                cell_pixels += 1;
                            }
                        }
                    }
                    ensure!(
                        cell_pixels > 0,
                        "story English transcription cell has no exact visible pixels: {context}"
                    );
                    protected_cells += 1;
                }
            }
            ensure!(
                protected_cells > 0,
                "mixed-language story surface has no fixed-cell English transcription: {context}"
            );
        }
        _ => anyhow::bail!("story surface is not Japanese-bearing: {context}"),
    }

    let japanese_text = visible_mask
        .iter()
        .zip(&protected_english)
        .map(|(visible, english)| *visible && !*english)
        .collect::<Vec<_>>();
    let protected_nontext = visible_mask
        .iter()
        .map(|visible| !*visible)
        .collect::<Vec<_>>();
    ensure!(
        (0..visible_mask.len()).all(|index| {
            usize::from(japanese_text[index])
                + usize::from(protected_english[index])
                + usize::from(protected_nontext[index])
                == 1
        }),
        "story Japanese, English, and non-text masks do not partition the source: {context}"
    );
    Ok(StoryVisiblePixelPartition {
        japanese_text,
        protected_english,
        protected_nontext,
    })
}

fn is_fixed_cell_english(character: char) -> bool {
    matches!(character, '\u{ff21}'..='\u{ff3a}' | '\u{ff41}'..='\u{ff5a}')
}

#[cfg(test)]
#[path = "story_dialogue_regions_tests.rs"]
mod story_dialogue_regions_tests;
