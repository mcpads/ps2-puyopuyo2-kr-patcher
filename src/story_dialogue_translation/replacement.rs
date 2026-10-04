use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use self::compositor::{
    StoryDialogueLinePlacement, clear_source_text, place_story_dialogue_line,
    verify_story_replacement_pixels,
};
use self::palette::select_story_dialogue_palette_indices;
use super::font::{
    TEXTURE_ROW_HEIGHT_PIXELS, load_story_dialogue_font, rasterize_story_dialogue_line,
};
use super::protected_english::{load_story_protected_english_mask, plan_protected_english_row};
use super::sequence::collect_story_dialogue_sequence_layouts;
use super::{StoryDialogueTranslationBundle, StoryDialogueTranslationEntry};
use crate::cpx::read_symbol_bytes;
use crate::executable::read_executable_symbol;
use crate::graphics_translation::raster::{read_binary_mask, write_binary_mask};
use crate::graphics_translation::{
    GraphicsTranslationBundle, GraphicsTranslationEntry, GraphicsTreatmentProfile,
};
use crate::indexed_texture::{
    colorize_psmt8, parse_file_records, parse_load_records, read_decoded_image, read_palette,
    sha256, unswizzle_psmt8_uploaded_as_psmct32, write_rgba_png,
};

mod build;
mod compositor;
mod palette;

pub(crate) use build::plan_story_dialogue_translation;
pub use build::{
    StoryDialogueTranslationBuildInputs, StoryDialogueTranslationBuildReport,
    build_story_dialogue_translation_development,
};

const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image3";

pub struct StoryDialogueReplacementInputs<'a> {
    pub executable: &'a [u8],
    pub image_container: &'a [u8],
    pub story: &'a StoryDialogueTranslationBundle,
    pub graphics: &'a GraphicsTranslationBundle,
    pub font_config_path: &'a Path,
    pub font_path: &'a Path,
    pub output_directory: &'a Path,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoryDialogueReplacementReport {
    pub schema: u32,
    pub status: &'static str,
    pub release_candidate_eligible: bool,
    pub font_id: String,
    pub font_sha256: String,
    pub surfaces: usize,
    pub rows: usize,
    pub source_text_pixels: usize,
    pub korean_text_pixels: usize,
    pub editable_union_pixels: usize,
    pub protected_pixels: usize,
    pub protected_english_pixels: usize,
    pub changed_pixels: usize,
    pub changed_protected_pixels: usize,
    pub changed_protected_english_pixels: usize,
    pub uncleared_source_only_pixels: usize,
    pub replacements: Vec<StoryDialogueSurfaceReplacement>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoryDialogueSurfaceReplacement {
    pub story_surface_id: String,
    pub consumer_table: String,
    pub image_file_record: usize,
    pub palette_file_record: usize,
    pub width: usize,
    pub height: usize,
    pub rows: usize,
    pub source_linear_indices_sha256: String,
    pub replacement_linear_indices_sha256: String,
    pub replacement_rgba_sha256: String,
    pub replacement_png_path: Option<String>,
    pub replacement_png_sha256: Option<String>,
    pub korean_text_mask_path: Option<String>,
    pub korean_text_mask_sha256: Option<String>,
    pub transparent_palette_index: u8,
    pub outline_palette_index: u8,
    pub fill_palette_index: u8,
    pub source_text_pixels: usize,
    pub korean_text_pixels: usize,
    pub editable_union_pixels: usize,
    pub protected_pixels: usize,
    pub protected_english_pixels: usize,
    pub changed_pixels: usize,
    pub changed_protected_pixels: usize,
    pub changed_protected_english_pixels: usize,
    pub uncleared_source_only_pixels: usize,
}

pub(crate) struct GeneratedStoryDialogueSurface {
    pub(crate) image_file_record: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) replacement_linear_indices: Vec<u8>,
}

pub(crate) struct GeneratedStoryDialogueReplacements {
    pub(crate) report: StoryDialogueReplacementReport,
    pub(crate) surfaces: Vec<GeneratedStoryDialogueSurface>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewFileOutput {
    Write,
    Omit,
}

pub fn render_story_dialogue_replacements(
    inputs: StoryDialogueReplacementInputs<'_>,
) -> Result<StoryDialogueReplacementReport> {
    Ok(generate_story_dialogue_replacements(inputs, PreviewFileOutput::Write)?.report)
}

pub(crate) fn generate_story_dialogue_replacements(
    inputs: StoryDialogueReplacementInputs<'_>,
    preview_file_output: PreviewFileOutput,
) -> Result<GeneratedStoryDialogueReplacements> {
    let font = load_story_dialogue_font(inputs.font_config_path, inputs.font_path)?;
    let layouts =
        collect_story_dialogue_sequence_layouts(inputs.executable, inputs.story, inputs.graphics)?;
    ensure!(
        layouts.len() == inputs.story.entries.len(),
        "story replacement layout population changed"
    );
    let file_list = read_executable_symbol(inputs.executable, IMAGE_FILE_LIST_SYMBOL)?;
    let file_records = parse_file_records(&file_list.bytes)?;
    let executable = object::File::parse(inputs.executable)
        .context("failed to parse story replacement executable")?;

    let mut report = StoryDialogueReplacementReport {
        schema: 1,
        status: match preview_file_output {
            PreviewFileOutput::Write => "pre-human-review story replacement previews generated",
            PreviewFileOutput::Omit => {
                "pre-human-review story replacements verified in memory without preview files"
            }
        },
        release_candidate_eligible: false,
        font_id: font.config.id.clone(),
        font_sha256: font.config.sha256.clone(),
        surfaces: 0,
        rows: 0,
        source_text_pixels: 0,
        korean_text_pixels: 0,
        editable_union_pixels: 0,
        protected_pixels: 0,
        protected_english_pixels: 0,
        changed_pixels: 0,
        changed_protected_pixels: 0,
        changed_protected_english_pixels: 0,
        uncleared_source_only_pixels: 0,
        replacements: Vec::new(),
        evidence_boundary: "These deterministic development previews use the exact source texture, its original palette, the current split story ledger, and the fixed font configuration. The editable region is the union of the source Japanese mask and generated Korean mask; every pixel outside that union is unchanged, and reviewed fixed-cell English pixels are preserved exactly. This does not prove CPX insertion, runtime display, language approval, visual approval, independent review, or release-candidate eligibility.",
    };

    let mut surfaces = Vec::with_capacity(inputs.story.entries.len());
    for (entry, layout) in inputs.story.entries.iter().zip(layouts) {
        let graphics_entry = inputs
            .graphics
            .entries
            .iter()
            .find(|candidate| candidate.source.source_surface_id == entry.source.source_surface_id)
            .with_context(|| {
                format!(
                    "story replacement has no graphics entry: {}",
                    entry.source.story_surface_id
                )
            })?;
        let (replacement, generated) =
            render_story_dialogue_surface(StoryDialogueSurfaceRenderInputs {
                image_container: inputs.image_container,
                executable: &executable,
                file_records: &file_records,
                entry,
                graphics: graphics_entry,
                physical_text_rows: layout.physical_text_rows,
                source_ink_width_pixels_by_row: layout.source_ink_width_pixels_by_row,
                font: &font,
                output_directory: inputs.output_directory,
                preview_file_output,
            })?;
        report.surfaces += 1;
        report.rows += replacement.rows;
        report.source_text_pixels += replacement.source_text_pixels;
        report.korean_text_pixels += replacement.korean_text_pixels;
        report.editable_union_pixels += replacement.editable_union_pixels;
        report.protected_pixels += replacement.protected_pixels;
        report.protected_english_pixels += replacement.protected_english_pixels;
        report.changed_pixels += replacement.changed_pixels;
        report.changed_protected_pixels += replacement.changed_protected_pixels;
        report.changed_protected_english_pixels += replacement.changed_protected_english_pixels;
        report.uncleared_source_only_pixels += replacement.uncleared_source_only_pixels;
        report.replacements.push(replacement);
        surfaces.push(generated);
    }
    ensure!(
        report.surfaces == inputs.story.entries.len()
            && report.rows
                == inputs
                    .story
                    .entries
                    .iter()
                    .flat_map(|entry| &entry.authoring.segments)
                    .map(|segment| segment.korean_text.split('\n').count())
                    .sum::<usize>()
            && report.changed_protected_pixels == 0
            && report.changed_protected_english_pixels == 0
            && report.uncleared_source_only_pixels == 0,
        "story replacement report does not cover the complete current ledger"
    );
    Ok(GeneratedStoryDialogueReplacements { report, surfaces })
}

struct StoryDialogueSurfaceRenderInputs<'a> {
    image_container: &'a [u8],
    executable: &'a object::File<'a>,
    file_records: &'a [crate::indexed_texture::FileRecord],
    entry: &'a StoryDialogueTranslationEntry,
    graphics: &'a GraphicsTranslationEntry,
    physical_text_rows: Vec<usize>,
    source_ink_width_pixels_by_row: std::collections::BTreeMap<usize, usize>,
    font: &'a super::font::LoadedStoryDialogueFont,
    output_directory: &'a Path,
    preview_file_output: PreviewFileOutput,
}

fn render_story_dialogue_surface(
    inputs: StoryDialogueSurfaceRenderInputs<'_>,
) -> Result<(
    StoryDialogueSurfaceReplacement,
    GeneratedStoryDialogueSurface,
)> {
    let entry = inputs.entry;
    let image_record = *inputs
        .file_records
        .get(entry.source.image_file_record)
        .context("story image file record is out of range")?;
    ensure!(
        image_record.width == entry.source.width && image_record.height == entry.source.height,
        "story image file record dimensions differ from the ledger"
    );
    let palette_file_record = paired_palette_file_record(inputs.executable, entry)?;
    let palette_record = *inputs
        .file_records
        .get(palette_file_record)
        .context("story palette file record is out of range")?;
    let width = usize::try_from(image_record.width)?;
    let height = usize::try_from(image_record.height)?;
    let source_swizzled = read_decoded_image(inputs.image_container, image_record)?;
    let image_byte_length = width
        .checked_mul(height)
        .context("story image dimensions overflow")?;
    let source_linear = unswizzle_psmt8_uploaded_as_psmct32(
        source_swizzled
            .get(..image_byte_length)
            .context("story decoded image is shorter than its dimensions")?,
        width,
        height,
    )?;
    ensure!(
        sha256(&source_linear) == entry.source.linear_indices_sha256,
        "story source indices differ from the translation ledger: {}",
        entry.source.story_surface_id
    );
    let palette = read_palette(inputs.image_container, palette_record, true)?;
    let source_rgba = colorize_psmt8(&source_linear, &palette);
    let source_masks = read_story_source_pixel_masks(inputs.graphics, width, height, &source_rgba)?;
    ensure!(
        source_masks
            .protected_english
            .iter()
            .enumerate()
            .filter(|(_, selected)| **selected)
            .all(|(pixel, _)| inputs
                .physical_text_rows
                .contains(&(pixel / width / TEXTURE_ROW_HEIGHT_PIXELS))),
        "story English mask selects a row outside the dialogue consumer"
    );
    let palette_indices = select_story_dialogue_palette_indices(&source_linear, &palette)?;

    let mut replacement_linear = source_linear.clone();
    clear_source_text(
        &mut replacement_linear,
        &source_masks.japanese_text,
        palette_indices.transparent,
    )?;
    let mut korean_text_mask = vec![false; source_linear.len()];
    let rows = entry
        .authoring
        .segments
        .iter()
        .flat_map(|segment| {
            segment
                .source_text
                .split('\n')
                .zip(segment.korean_text.split('\n'))
        })
        .collect::<Vec<_>>();
    ensure!(
        rows.len() == inputs.physical_text_rows.len(),
        "story replacement rows differ from physical text rows"
    );
    for ((source_text, korean_text), texture_row) in rows.iter().zip(&inputs.physical_text_rows) {
        let source_ink_width = *inputs
            .source_ink_width_pixels_by_row
            .get(texture_row)
            .context("story replacement row has no source ink width")?;
        let y = texture_row
            .checked_mul(TEXTURE_ROW_HEIGHT_PIXELS)
            .context("story replacement vertical placement overflow")?;
        let plan = plan_protected_english_row(
            source_text,
            korean_text,
            &source_masks.protected_english,
            width,
            height,
            *texture_row,
        )?;
        for slot in plan.fragment_slots(source_ink_width) {
            if slot
                .text
                .chars()
                .any(|character| !character.is_whitespace())
            {
                let line = rasterize_story_dialogue_line(inputs.font, slot.text)?;
                let fragment_right = slot
                    .x
                    .checked_add(line.width)
                    .context("story replacement fragment width overflow")?;
                ensure!(
                    fragment_right <= slot.right_limit && line.height <= TEXTURE_ROW_HEIGHT_PIXELS,
                    "story replacement fragment exceeds its exact source row: {}/row {}",
                    entry.source.story_surface_id,
                    texture_row
                );
                let centered_y = y
                    .checked_add((TEXTURE_ROW_HEIGHT_PIXELS - line.height) / 2)
                    .context("story replacement vertical centering overflow")?;
                place_story_dialogue_line(
                    &mut replacement_linear,
                    &mut korean_text_mask,
                    StoryDialogueLinePlacement {
                        surface_width: width,
                        surface_height: height,
                        x: slot.x,
                        y: centered_y,
                        line: &line,
                        outline_index: palette_indices.outline,
                        fill_index: palette_indices.fill,
                        protected_pixel_mask: &source_masks.protected_english,
                    },
                )?;
            }
        }
    }
    let pixel_counts = verify_story_replacement_pixels(
        &source_linear,
        &replacement_linear,
        &source_masks.japanese_text,
        &korean_text_mask,
        &source_masks.protected_english,
        palette_indices.transparent,
    )?;
    let replacement_rgba = colorize_psmt8(&replacement_linear, &palette);
    let basename = format!("demo-{:02X}", entry.source.consumer_sequence);
    let replacement_path = inputs
        .output_directory
        .join(format!("{basename}-replacement.png"));
    let korean_mask_path = inputs
        .output_directory
        .join(format!("{basename}-korean.mask.png"));
    let (
        replacement_png_path,
        replacement_png_sha256,
        korean_text_mask_path,
        korean_text_mask_sha256,
    ) = if inputs.preview_file_output == PreviewFileOutput::Write {
        write_rgba_png(
            &replacement_path,
            image_record.width,
            image_record.height,
            &replacement_rgba,
        )?;
        write_binary_mask(&korean_mask_path, width, height, &korean_text_mask)?;
        (
            Some(path_string(&replacement_path)?),
            Some(hash_file(&replacement_path)?),
            Some(path_string(&korean_mask_path)?),
            Some(hash_file(&korean_mask_path)?),
        )
    } else {
        (None, None, None, None)
    };

    let report = StoryDialogueSurfaceReplacement {
        story_surface_id: entry.source.story_surface_id.clone(),
        consumer_table: entry.source.consumer_table.clone(),
        image_file_record: entry.source.image_file_record,
        palette_file_record,
        width,
        height,
        rows: rows.len(),
        source_linear_indices_sha256: sha256(&source_linear),
        replacement_linear_indices_sha256: sha256(&replacement_linear),
        replacement_rgba_sha256: sha256(&replacement_rgba),
        replacement_png_path,
        replacement_png_sha256,
        korean_text_mask_path,
        korean_text_mask_sha256,
        transparent_palette_index: palette_indices.transparent,
        outline_palette_index: palette_indices.outline,
        fill_palette_index: palette_indices.fill,
        source_text_pixels: source_masks
            .japanese_text
            .iter()
            .filter(|selected| **selected)
            .count(),
        korean_text_pixels: korean_text_mask
            .iter()
            .filter(|selected| **selected)
            .count(),
        editable_union_pixels: pixel_counts.editable_pixels,
        protected_pixels: pixel_counts.protected_pixels,
        protected_english_pixels: pixel_counts.protected_english_pixels,
        changed_pixels: pixel_counts.changed_pixels,
        changed_protected_pixels: pixel_counts.changed_protected_pixels,
        changed_protected_english_pixels: pixel_counts.changed_protected_english_pixels,
        uncleared_source_only_pixels: pixel_counts.uncleared_source_only_pixels,
    };
    let generated = GeneratedStoryDialogueSurface {
        image_file_record: entry.source.image_file_record,
        width,
        height,
        replacement_linear_indices: replacement_linear,
    };
    Ok((report, generated))
}

fn paired_palette_file_record(
    executable: &object::File<'_>,
    entry: &StoryDialogueTranslationEntry,
) -> Result<usize> {
    let (_, bytes) = read_symbol_bytes(executable, &entry.source.consumer_table)?;
    let loads = parse_load_records(&bytes)?;
    let images = loads
        .iter()
        .filter(|record| record.operation == 1)
        .collect::<Vec<_>>();
    let palettes = loads
        .iter()
        .filter(|record| record.operation == 2)
        .collect::<Vec<_>>();
    ensure!(
        !images.is_empty() && images.len() == palettes.len(),
        "story load table does not contain paired images and palettes"
    );
    let matching = images
        .iter()
        .enumerate()
        .filter(|(_, record)| record.file_record == entry.source.image_file_record)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    ensure!(
        matching.len() == 1,
        "story image does not have one exact palette pair"
    );
    Ok(palettes[matching[0]].file_record)
}

struct StorySourcePixelMasks {
    japanese_text: Vec<bool>,
    protected_english: Vec<bool>,
}

fn read_story_source_pixel_masks(
    graphics: &GraphicsTranslationEntry,
    width: usize,
    height: usize,
    source_rgba: &[u8],
) -> Result<StorySourcePixelMasks> {
    ensure!(
        graphics.authoring.regions.len() == 1
            && graphics.authoring.regions[0].treatment_profile
                == GraphicsTreatmentProfile::CutsceneBakedText,
        "story graphics entry does not contain one dialogue region"
    );
    let source_asset = &graphics.authoring.regions[0].source_pixel_mask;
    ensure!(
        hash_file(Path::new(&source_asset.path))? == source_asset.sha256,
        "story source mask SHA-256 differs from the graphics ledger"
    );
    let japanese_text = read_binary_mask(
        Path::new(&source_asset.path),
        width,
        height,
        &graphics.source.source_surface_id,
    )?;
    ensure!(
        source_rgba.len() == width * height * 4,
        "story source mask dimensions differ from the source texture"
    );
    let protected_english = load_story_protected_english_mask(graphics, width, height)?;
    ensure!(
        graphics.authoring.protected_nontext_masks.len() == 1,
        "story graphics entry does not contain one protected non-text mask"
    );
    let nontext_asset = &graphics.authoring.protected_nontext_masks[0];
    ensure!(
        hash_file(Path::new(&nontext_asset.path))? == nontext_asset.sha256,
        "story protected non-text mask SHA-256 differs from the graphics ledger"
    );
    let protected_nontext = read_binary_mask(
        Path::new(&nontext_asset.path),
        width,
        height,
        &graphics.source.source_surface_id,
    )?;
    ensure!(
        (0..width * height).all(|pixel| {
            let visible = source_rgba[pixel * 4 + 3] != 0;
            usize::from(japanese_text[pixel])
                + usize::from(protected_english[pixel])
                + usize::from(protected_nontext[pixel])
                == 1
                && (japanese_text[pixel] || protected_english[pixel]) == visible
        }),
        "story Japanese, English, and non-text masks do not partition exact source pixels"
    );
    Ok(StorySourcePixelMasks {
        japanese_text,
        protected_english,
    })
}

fn hash_file(path: &Path) -> Result<String> {
    Ok(sha256(&fs::read(path).with_context(|| {
        format!("failed to read {}", path.display())
    })?))
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(ToOwned::to_owned)
        .with_context(|| format!("output path is not UTF-8: {}", path.display()))
}

#[cfg(test)]
#[path = "replacement_tests.rs"]
mod replacement_tests;
