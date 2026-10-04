use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

use crate::cpx::{CPX_BLOCK_ALIGNMENT, decode_cpx_block, read_u32_at};

const LOAD_RECORD_BYTES: usize = 16;
const FILE_RECORD_BYTES: usize = 20;
const TEXTURE_REGISTRATION_PAIR_BYTES: usize = 8;
const PALETTE_BYTES: usize = 256 * 4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct LoadRecord {
    pub(crate) flags: u32,
    pub(crate) operation: u32,
    pub(crate) file_record: usize,
    pub(crate) destination: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FileRecord {
    pub(crate) sector_offset: u32,
    pub(crate) sector_count: u32,
    pub(crate) stored_byte_length: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct TextureRegistrationPair {
    pub(crate) image_load_index: usize,
    pub(crate) palette_slot: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct RenderedIndexedTexture {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) decoded_trailing_byte_length: u64,
    pub(crate) swizzled_indices_sha256: String,
    pub(crate) linear_indices_sha256: String,
    pub(crate) uploaded_palette_sha256: String,
    pub(crate) rgba_sha256: String,
    pub(crate) png_path: String,
    pub(crate) png_sha256: String,
}

pub(crate) fn parse_load_records(source: &[u8]) -> Result<Vec<LoadRecord>> {
    ensure!(
        source.len().is_multiple_of(LOAD_RECORD_BYTES),
        "texture load table is not a sequence of 16-byte records"
    );
    let mut records = Vec::new();
    let mut terminated = false;
    for record in source.chunks_exact(LOAD_RECORD_BYTES) {
        let flags = read_u32_at(record, 0)?;
        if flags == u32::MAX {
            ensure!(
                record[4..].iter().all(|byte| *byte == 0),
                "texture load terminator has nonzero fields"
            );
            terminated = true;
            continue;
        }
        ensure!(!terminated, "texture load record follows the terminator");
        records.push(LoadRecord {
            flags,
            operation: read_u32_at(record, 4)?,
            file_record: usize::try_from(read_u32_at(record, 8)?)?,
            destination: read_u32_at(record, 12)?,
        });
    }
    ensure!(terminated, "texture load table has no terminator");
    Ok(records)
}

pub(crate) fn parse_file_records(source: &[u8]) -> Result<Vec<FileRecord>> {
    ensure!(
        source.len().is_multiple_of(FILE_RECORD_BYTES),
        "image file list is not a sequence of 20-byte records"
    );
    source
        .chunks_exact(FILE_RECORD_BYTES)
        .map(|record| {
            Ok(FileRecord {
                sector_offset: read_u32_at(record, 0)?,
                sector_count: read_u32_at(record, 4)?,
                stored_byte_length: read_u32_at(record, 8)?,
                width: read_u32_at(record, 12)?,
                height: read_u32_at(record, 16)?,
            })
        })
        .collect()
}

pub(crate) fn parse_texture_registration_pairs(
    symbol: &str,
    source: &[u8],
) -> Result<Vec<TextureRegistrationPair>> {
    ensure!(
        source.len().is_multiple_of(TEXTURE_REGISTRATION_PAIR_BYTES),
        "{symbol} is not a sequence of eight-byte pairs"
    );
    let mut pairs = Vec::new();
    let mut terminated = false;
    for record in source.chunks_exact(TEXTURE_REGISTRATION_PAIR_BYTES) {
        let image = read_u32_at(record, 0)?;
        let palette = read_u32_at(record, 4)?;
        if image == u32::MAX {
            ensure!(palette == u32::MAX, "{symbol} has a partial terminator");
            terminated = true;
            continue;
        }
        ensure!(!terminated, "{symbol} has a pair after its terminator");
        ensure!(palette != u32::MAX, "{symbol} has a partial pair");
        pairs.push(TextureRegistrationPair {
            image_load_index: usize::try_from(image)?,
            palette_slot: usize::try_from(palette)?,
        });
    }
    ensure!(terminated, "{symbol} has no terminator");
    Ok(pairs)
}

pub(crate) fn parse_image_sentinel_texture_registration_pairs(
    symbol: &str,
    source: &[u8],
) -> Result<Vec<TextureRegistrationPair>> {
    ensure!(
        source.len().is_multiple_of(4),
        "{symbol} is not a sequence of 32-bit registration fields"
    );
    let mut pairs = Vec::new();
    let mut offset = 0;
    while offset < source.len() {
        let image = read_u32_at(source, offset)?;
        offset += 4;
        if image == u32::MAX {
            ensure!(
                source[offset..]
                    .chunks_exact(4)
                    .all(
                        |word| u32::from_le_bytes(word.try_into().expect("four-byte word"))
                            == u32::MAX
                    ),
                "{symbol} has fields after its image sentinel"
            );
            return Ok(pairs);
        }
        ensure!(
            offset + 4 <= source.len(),
            "{symbol} ends between an image and palette field"
        );
        let palette = read_u32_at(source, offset)?;
        offset += 4;
        ensure!(palette != u32::MAX, "{symbol} has a partial pair");
        pairs.push(TextureRegistrationPair {
            image_load_index: usize::try_from(image)?,
            palette_slot: usize::try_from(palette)?,
        });
    }
    anyhow::bail!("{symbol} has no image sentinel")
}

fn record_payload(container: &[u8], record: FileRecord) -> Result<&[u8]> {
    let offset = usize::try_from(record.sector_offset)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("image record offset overflow")?;
    let slot_length = usize::try_from(record.sector_count)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("image record slot length overflow")?;
    let stored_length = usize::try_from(record.stored_byte_length)?;
    ensure!(
        stored_length <= slot_length,
        "image record payload exceeds its sector slot"
    );
    let end = offset
        .checked_add(stored_length)
        .context("image record payload range overflow")?;
    container
        .get(offset..end)
        .context("image record payload exceeds the container")
}

pub(crate) fn read_decoded_image(container: &[u8], record: FileRecord) -> Result<Vec<u8>> {
    ensure!(
        record.width > 0 && record.height > 0,
        "image file record has no dimensions"
    );
    let offset = usize::try_from(record.sector_offset)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("image offset overflow")?;
    let source = container
        .get(offset..)
        .context("image offset exceeds the container")?;
    let decoded = decode_cpx_block(source)?;
    let slot_length = usize::try_from(record.sector_count)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("image slot length overflow")?;
    let slot = source
        .get(..slot_length)
        .context("image slot exceeds the container")?;
    ensure!(
        decoded.encoded_byte_length == usize::try_from(record.stored_byte_length)? + 1,
        "CPX length differs from its image file record"
    );
    ensure!(
        decoded.encoded_byte_length <= slot.len()
            && slot[decoded.encoded_byte_length..]
                .iter()
                .all(|byte| *byte == 0),
        "CPX image slot has nonzero bytes after the encoded stream"
    );
    ensure!(
        u64::from(decoded.header_value) == u64::from(record.width) * u64::from(record.height),
        "CPX header differs from its image file-record dimensions"
    );
    Ok(decoded.bytes)
}

pub fn decode_linear_indices_at_cpx_offset(
    container: &[u8],
    cpx_offset: u64,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let offset = usize::try_from(cpx_offset)?;
    let source = container
        .get(offset..)
        .context("CPX image offset exceeds the container")?;
    let decoded = decode_cpx_block(source)?;
    let image_byte_length = usize::try_from(u64::from(width) * u64::from(height))?;
    ensure!(
        width > 0
            && height > 0
            && u64::from(decoded.header_value) == u64::from(width) * u64::from(height)
            && decoded.bytes.len() >= image_byte_length,
        "CPX image dimensions differ from the requested linear indices"
    );
    unswizzle_psmt8_uploaded_as_psmct32(
        &decoded.bytes[..image_byte_length],
        usize::try_from(width)?,
        usize::try_from(height)?,
    )
}

pub(crate) fn read_palette(
    container: &[u8],
    record: FileRecord,
    clear_first_alpha: bool,
) -> Result<Vec<u8>> {
    ensure!(
        record.width == 0 && record.height == 0,
        "palette file record unexpectedly has image dimensions"
    );
    let mut palette = record_payload(container, record)?.to_vec();
    ensure!(
        palette.len() == PALETTE_BYTES,
        "palette is not 256 RGBA32 entries"
    );
    if clear_first_alpha {
        palette[3] = 0;
    }
    Ok(palette)
}

pub(crate) fn render_indexed_texture(
    container: &[u8],
    image_record: FileRecord,
    palette_record: FileRecord,
    clear_first_alpha: bool,
    output_path: &Path,
) -> Result<RenderedIndexedTexture> {
    let decoded = read_decoded_image(container, image_record)?;
    let image_byte_length =
        usize::try_from(u64::from(image_record.width) * u64::from(image_record.height))?;
    ensure!(
        decoded.len() >= image_byte_length,
        "decoded indexed texture is shorter than its dimensions"
    );
    let swizzled_indices = &decoded[..image_byte_length];
    let linear_indices = unswizzle_psmt8_uploaded_as_psmct32(
        swizzled_indices,
        usize::try_from(image_record.width)?,
        usize::try_from(image_record.height)?,
    )?;
    let uploaded_palette = read_palette(container, palette_record, clear_first_alpha)?;
    let rgba = colorize_psmt8(&linear_indices, &uploaded_palette);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    write_rgba_png(output_path, image_record.width, image_record.height, &rgba)?;
    let png_bytes = fs::read(output_path)
        .with_context(|| format!("failed to hash {}", output_path.display()))?;
    Ok(RenderedIndexedTexture {
        width: image_record.width,
        height: image_record.height,
        decoded_trailing_byte_length: u64::try_from(decoded.len() - image_byte_length)?,
        swizzled_indices_sha256: sha256(swizzled_indices),
        linear_indices_sha256: sha256(&linear_indices),
        uploaded_palette_sha256: sha256(&uploaded_palette),
        rgba_sha256: sha256(&rgba),
        png_path: output_path
            .to_str()
            .with_context(|| format!("output path is not UTF-8: {}", output_path.display()))?
            .to_string(),
        png_sha256: sha256(&png_bytes),
    })
}

pub(crate) fn unswizzle_psmt8_uploaded_as_psmct32(
    source: &[u8],
    width: usize,
    height: usize,
) -> Result<Vec<u8>> {
    ensure!(
        width > 0 && width.is_multiple_of(16),
        "PSMT8 width is not a positive multiple of 16"
    );
    ensure!(
        height > 0 && height.is_multiple_of(4),
        "PSMT8 height is not a positive multiple of 4"
    );
    ensure!(
        source.len() == width.checked_mul(height).context("PSMT8 size overflow")?,
        "PSMT8 byte length differs from its dimensions"
    );
    let mut output = vec![0; source.len()];
    for y in 0..height {
        for x in 0..width {
            let block_location = (y & !0xf) * width + (x & !0xf) * 2;
            let swap_selector = (((y + 2) >> 2) & 1) * 4;
            let position_y = (((y & !3) >> 1) + (y & 1)) & 7;
            let column_location = position_y * width * 2 + ((x + swap_selector) & 7) * 4;
            let byte_number = ((y >> 1) & 1) + ((x >> 2) & 2);
            let source_index = block_location + column_location + byte_number;
            output[y * width + x] = *source
                .get(source_index)
                .with_context(|| format!("PSMT8 swizzle address exceeds input at ({x}, {y})"))?;
        }
    }
    Ok(output)
}

pub(crate) fn swizzle_psmt8_uploaded_as_psmct32(
    source: &[u8],
    width: usize,
    height: usize,
) -> Result<Vec<u8>> {
    ensure!(
        width > 0 && width.is_multiple_of(16),
        "PSMT8 width is not a positive multiple of 16"
    );
    ensure!(
        height > 0 && height.is_multiple_of(4),
        "PSMT8 height is not a positive multiple of 4"
    );
    ensure!(
        source.len() == width.checked_mul(height).context("PSMT8 size overflow")?,
        "PSMT8 byte length differs from its dimensions"
    );
    let mut output = vec![0; source.len()];
    let mut visited = vec![false; source.len()];
    for y in 0..height {
        for x in 0..width {
            let block_location = (y & !0xf) * width + (x & !0xf) * 2;
            let swap_selector = (((y + 2) >> 2) & 1) * 4;
            let position_y = (((y & !3) >> 1) + (y & 1)) & 7;
            let column_location = position_y * width * 2 + ((x + swap_selector) & 7) * 4;
            let byte_number = ((y >> 1) & 1) + ((x >> 2) & 2);
            let destination = block_location + column_location + byte_number;
            ensure!(
                destination < output.len() && !visited[destination],
                "PSMT8 swizzle address is invalid at ({x}, {y})"
            );
            output[destination] = source[y * width + x];
            visited[destination] = true;
        }
    }
    ensure!(
        visited.into_iter().all(|was_written| was_written),
        "PSMT8 swizzle did not cover the destination"
    );
    Ok(output)
}

pub(crate) fn colorize_psmt8(indices: &[u8], uploaded_palette: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(indices.len() * 4);
    for &index in indices {
        let logical_index = usize::from(index);
        let stored_index = match logical_index & 0x18 {
            0x08 => logical_index + 8,
            0x10 => logical_index - 8,
            _ => logical_index,
        };
        let offset = stored_index * 4;
        rgba.extend_from_slice(&uploaded_palette[offset..offset + 3]);
        let gs_alpha = u16::from(uploaded_palette[offset + 3]).min(0x80);
        rgba.push(((gs_alpha * 255 + 64) / 128) as u8);
    }
    rgba
}

pub(crate) fn map_rgba_to_source_palette(
    source_indices: &[u8],
    source_rgba: &[u8],
    replacement_rgba: &[u8],
    palette: &[u8],
) -> Result<(Vec<u8>, usize, usize)> {
    ensure!(
        source_rgba.len() == replacement_rgba.len()
            && source_rgba.len() == source_indices.len() * 4,
        "replacement RGBA and index dimensions differ"
    );
    let mut source_color_indices = BTreeMap::<[u8; 4], u8>::new();
    for &index in source_indices {
        let color: [u8; 4] = colorize_psmt8(&[index], palette)
            .try_into()
            .expect("one palette index produces one RGBA pixel");
        source_color_indices
            .entry(color)
            .and_modify(|current| *current = (*current).min(index))
            .or_insert(index);
    }
    let mut replacement_indices = source_indices.to_vec();
    let mut changed = 0_usize;
    let mut preserved = 0_usize;
    for (pixel, (source, replacement)) in source_rgba
        .chunks_exact(4)
        .zip(replacement_rgba.chunks_exact(4))
        .enumerate()
    {
        if source == replacement {
            preserved += 1;
            continue;
        }
        let color: [u8; 4] = replacement
            .try_into()
            .expect("RGBA chunks have exactly four bytes");
        replacement_indices[pixel] = *source_color_indices.get(&color).with_context(|| {
            format!("replacement pixel {pixel} uses a color absent from source")
        })?;
        changed += 1;
    }
    Ok((replacement_indices, changed, preserved))
}

pub(crate) fn write_rgba_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    ensure!(
        rgba.len() == usize::try_from(u64::from(width) * u64::from(height) * 4)?,
        "RGBA byte length differs from its dimensions"
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .with_context(|| format!("failed to write PNG header to {}", path.display()))?;
    writer
        .write_image_data(rgba)
        .with_context(|| format!("failed to write PNG pixels to {}", path.display()))?;
    Ok(())
}

pub(crate) fn sha256(source: &[u8]) -> String {
    format!("{:x}", Sha256::digest(source))
}

#[cfg(test)]
#[path = "indexed_texture_tests.rs"]
mod indexed_texture_tests;
