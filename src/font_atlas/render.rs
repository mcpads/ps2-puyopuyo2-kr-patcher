use std::path::Path;

use anyhow::Result;

use super::RenderedFontAtlas;
use crate::indexed_texture::{FileRecord, render_indexed_texture};

#[allow(clippy::too_many_arguments)]
pub(super) fn render_indexed_font_atlas(
    image_container: &[u8],
    image_record: FileRecord,
    palette_record: FileRecord,
    clear_first_alpha: bool,
    output_path: &Path,
    load_table_image_count: usize,
    load_table_palette_count: usize,
    image_load_index: usize,
    palette_slot: usize,
    image_file_record: usize,
    palette_file_record: usize,
) -> Result<RenderedFontAtlas> {
    let rendered = render_indexed_texture(
        image_container,
        image_record,
        palette_record,
        clear_first_alpha,
        output_path,
    )?;

    Ok(RenderedFontAtlas {
        load_table_image_count,
        load_table_palette_count,
        image_load_index,
        palette_slot,
        image_file_record,
        palette_file_record,
        width: rendered.width,
        height: rendered.height,
        decoded_trailing_byte_length: rendered.decoded_trailing_byte_length,
        swizzled_indices_sha256: rendered.swizzled_indices_sha256,
        linear_indices_sha256: rendered.linear_indices_sha256,
        uploaded_palette_sha256: rendered.uploaded_palette_sha256,
        rgba_sha256: rendered.rgba_sha256,
        png_path: rendered.png_path,
        png_sha256: rendered.png_sha256,
    })
}
