use anyhow::{Context, Result, ensure};

use crate::cpx::read_u32_at;

const PATTERN_RECORD_BYTES: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TexturePatternRecord {
    pub(crate) image_load_index: usize,
    pub(crate) source_x: u32,
    pub(crate) source_y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

pub(crate) fn parse_pattern_records(
    symbol: &str,
    source: &[u8],
) -> Result<Vec<TexturePatternRecord>> {
    ensure!(
        source.len().is_multiple_of(PATTERN_RECORD_BYTES),
        "{symbol} is not a sequence of 24-byte pattern records"
    );
    source
        .chunks_exact(PATTERN_RECORD_BYTES)
        .map(|record| {
            ensure!(
                read_u32_at(record, 0)? == 0,
                "{symbol} pattern flags changed"
            );
            Ok(TexturePatternRecord {
                image_load_index: usize::try_from(read_u32_at(record, 4)?)?,
                source_x: read_u32_at(record, 8)?,
                source_y: read_u32_at(record, 12)?,
                width: read_u32_at(record, 16)?,
                height: read_u32_at(record, 20)?,
            })
        })
        .collect()
}

pub(crate) fn validate_pattern_indices(
    symbol: &str,
    records: &[TexturePatternRecord],
    expected_indices: &[usize],
) -> Result<()> {
    let actual = records
        .iter()
        .map(|record| record.image_load_index)
        .collect::<Vec<_>>();
    ensure!(
        actual == expected_indices,
        "{symbol} texture indices changed"
    );
    Ok(())
}

pub(crate) fn validate_pattern_rectangles<'a>(
    role: &str,
    image_load_index: usize,
    image_width: u32,
    image_height: u32,
    records: impl Iterator<Item = &'a TexturePatternRecord>,
) -> Result<()> {
    for record in records.filter(|record| record.image_load_index == image_load_index) {
        let right = record
            .source_x
            .checked_add(record.width)
            .with_context(|| format!("{role} horizontal range overflow"))?;
        let bottom = record
            .source_y
            .checked_add(record.height)
            .with_context(|| format!("{role} vertical range overflow"))?;
        ensure!(
            record.width > 0 && record.height > 0 && right <= image_width && bottom <= image_height,
            "{role} rectangle exceeds image load {image_load_index}"
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "texture_pattern/texture_pattern_tests.rs"]
mod texture_pattern_tests;
