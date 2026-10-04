use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::font::TEXTURE_ROW_HEIGHT_PIXELS;
use crate::graphics_translation::GraphicsTranslationEntry;
use crate::graphics_translation::raster::read_binary_mask;
use crate::indexed_texture::sha256;
use crate::texture_language_survey::TextureLanguageClassification;

const STORY_TEXTURE_CELL_WIDTH_PIXELS: usize = 24;

#[derive(Debug, Eq, PartialEq)]
struct ProtectedEnglishTextRun {
    text: String,
    character_cells: std::ops::Range<usize>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ProtectedEnglishRowPlan {
    korean_fragments: Vec<String>,
    protected_cell_spans: Vec<std::ops::Range<usize>>,
}

pub(super) struct ProtectedEnglishFragmentSlot<'a> {
    pub(super) text: &'a str,
    pub(super) x: usize,
    pub(super) right_limit: usize,
}

impl ProtectedEnglishRowPlan {
    pub(super) fn fragment_slots(
        &self,
        source_envelope_width: usize,
    ) -> Vec<ProtectedEnglishFragmentSlot<'_>> {
        let final_right = self
            .protected_cell_spans
            .last()
            .map_or(source_envelope_width, |span| {
                source_envelope_width.max(span.end)
            });
        let mut x = 0_usize;
        self.korean_fragments
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let slot = ProtectedEnglishFragmentSlot {
                    text,
                    x,
                    right_limit: self
                        .protected_cell_spans
                        .get(index)
                        .map_or(final_right, |span| span.start),
                };
                if let Some(span) = self.protected_cell_spans.get(index) {
                    x = span.end;
                }
                slot
            })
            .collect()
    }

    pub(super) fn protected_right_edge(&self) -> usize {
        self.protected_cell_spans.last().map_or(0, |span| span.end)
    }
}

pub(super) fn load_story_protected_english_mask(
    graphics: &GraphicsTranslationEntry,
    width: usize,
    height: usize,
) -> Result<Vec<bool>> {
    if graphics.source.requires_english_pixel_preservation {
        ensure!(
            graphics.source.classification == TextureLanguageClassification::JapaneseEnglishMixed,
            "story English protection flag disagrees with its language classification"
        );
        let asset = graphics
            .authoring
            .protected_english_mask
            .as_ref()
            .context("mixed story surface has no protected English mask")?;
        let bytes = std::fs::read(&asset.path)
            .with_context(|| format!("failed to read story English mask {}", asset.path))?;
        ensure!(
            sha256(&bytes) == asset.sha256,
            "story English mask SHA-256 differs from the graphics ledger"
        );
        let mask = read_binary_mask(
            Path::new(&asset.path),
            width,
            height,
            &graphics.source.source_surface_id,
        )?;
        ensure!(
            mask.iter().any(|selected| *selected),
            "story English mask contains no protected pixels"
        );
        Ok(mask)
    } else {
        ensure!(
            graphics.source.classification == TextureLanguageClassification::JapaneseOnly
                && graphics.authoring.protected_english_mask.is_none(),
            "Japanese-only story surface declares English protection"
        );
        Ok(vec![false; width * height])
    }
}

pub(super) fn plan_protected_english_row(
    source_text: &str,
    korean_text: &str,
    protected_english_mask: &[bool],
    surface_width: usize,
    surface_height: usize,
    texture_row: usize,
) -> Result<ProtectedEnglishRowPlan> {
    let protected_cell_spans = protected_english_cell_spans(
        protected_english_mask,
        surface_width,
        surface_height,
        texture_row,
    )?;
    let (source_fragments, source_runs) = split_fixed_cell_english(source_text);
    let (korean_fragments, korean_runs) = split_fixed_cell_english(korean_text);
    ensure!(
        source_fragments.len() == source_runs.len() + 1
            && korean_fragments.len() == korean_runs.len() + 1,
        "story protected English text split changed"
    );
    ensure!(
        source_runs.len() == protected_cell_spans.len()
            && korean_runs.len() == protected_cell_spans.len(),
        "story protected English mask does not match source and Korean text runs"
    );
    for ((source_run, korean_run), span) in source_runs
        .iter()
        .zip(&korean_runs)
        .zip(&protected_cell_spans)
    {
        ensure!(
            source_run.text == korean_run.text
                && source_run.character_cells.start * STORY_TEXTURE_CELL_WIDTH_PIXELS == span.start
                && source_run.character_cells.end * STORY_TEXTURE_CELL_WIDTH_PIXELS == span.end,
            "story protected English text or fixed source cells differ"
        );
    }
    Ok(ProtectedEnglishRowPlan {
        korean_fragments,
        protected_cell_spans,
    })
}

fn protected_english_cell_spans(
    mask: &[bool],
    surface_width: usize,
    surface_height: usize,
    texture_row: usize,
) -> Result<Vec<std::ops::Range<usize>>> {
    ensure!(
        mask.len() == surface_width * surface_height,
        "story English mask dimensions changed"
    );
    let top = texture_row
        .checked_mul(TEXTURE_ROW_HEIGHT_PIXELS)
        .context("story English texture row overflow")?;
    let bottom = top
        .checked_add(TEXTURE_ROW_HEIGHT_PIXELS)
        .context("story English texture row height overflow")?
        .min(surface_height);
    ensure!(top < bottom, "story English texture row is out of range");
    let cells = (0..surface_width.div_ceil(STORY_TEXTURE_CELL_WIDTH_PIXELS))
        .filter(|cell| {
            let left = cell * STORY_TEXTURE_CELL_WIDTH_PIXELS;
            let right = (left + STORY_TEXTURE_CELL_WIDTH_PIXELS).min(surface_width);
            (top..bottom).any(|y| (left..right).any(|x| mask[y * surface_width + x]))
        })
        .collect::<Vec<_>>();
    let mut spans = Vec::<std::ops::Range<usize>>::new();
    for cell in cells {
        match spans.last_mut() {
            Some(span) if span.end == cell * STORY_TEXTURE_CELL_WIDTH_PIXELS => {
                span.end = ((cell + 1) * STORY_TEXTURE_CELL_WIDTH_PIXELS).min(surface_width);
            }
            _ => spans.push(
                cell * STORY_TEXTURE_CELL_WIDTH_PIXELS
                    ..((cell + 1) * STORY_TEXTURE_CELL_WIDTH_PIXELS).min(surface_width),
            ),
        }
    }
    Ok(spans)
}

fn split_fixed_cell_english(text: &str) -> (Vec<String>, Vec<ProtectedEnglishTextRun>) {
    let characters = text.chars().collect::<Vec<_>>();
    let mut fragments = Vec::new();
    let mut runs = Vec::new();
    let mut cursor = 0_usize;
    while cursor < characters.len() {
        let Some(relative_start) = characters[cursor..]
            .iter()
            .position(|character| is_fixed_cell_english(*character))
        else {
            break;
        };
        let start = cursor + relative_start;
        fragments.push(characters[cursor..start].iter().collect());
        let end = characters[start..]
            .iter()
            .position(|character| !is_fixed_cell_english(*character))
            .map_or(characters.len(), |relative_end| start + relative_end);
        runs.push(ProtectedEnglishTextRun {
            text: characters[start..end].iter().collect(),
            character_cells: start..end,
        });
        cursor = end;
    }
    fragments.push(characters[cursor..].iter().collect());
    (fragments, runs)
}

fn is_fixed_cell_english(character: char) -> bool {
    matches!(character, '\u{ff21}'..='\u{ff3a}' | '\u{ff41}'..='\u{ff5a}')
}

#[cfg(test)]
pub(super) fn plan_protected_english_row_for_test(
    source_text: &str,
    korean_text: &str,
    protected_english_mask: &[bool],
    surface_width: usize,
    surface_height: usize,
    texture_row: usize,
    source_envelope_width: usize,
) -> Result<Vec<(String, usize, usize)>> {
    Ok(plan_protected_english_row(
        source_text,
        korean_text,
        protected_english_mask,
        surface_width,
        surface_height,
        texture_row,
    )?
    .fragment_slots(source_envelope_width)
    .into_iter()
    .map(|slot| (slot.text.to_string(), slot.x, slot.right_limit))
    .collect())
}
