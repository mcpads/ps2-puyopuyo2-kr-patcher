use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::Serialize;

use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::{
    parse_file_records, parse_load_records, parse_texture_registration_pairs,
    render_indexed_texture,
};
use crate::texture_pattern::{TexturePatternRecord, parse_pattern_records};
use crate::texture_source::TextureSourceInputs;

const TITLE_LOAD_TABLE_SYMBOL: &str = "TitleTexTbl";
const TITLE_TEXTURE_SET_SYMBOLS: [&str; 3] = ["TitleTex_0", "TitleTex_1", "TitleTex_2"];
const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image";
const TITLE_PATTERN_SYMBOL: &str = "TitlePtn";

#[derive(Clone, Debug, Serialize)]
pub struct TitleTextureCorpus {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub image_container_iso_path: String,
    pub image_container_sha256: String,
    pub load_table_symbol: String,
    pub file_list_symbol: String,
    pub texture_sets: Vec<TitleTextureSet>,
    pub pattern_symbol: String,
    pub patterns: Vec<TitlePatternRecord>,
    pub summary: TitleTextureSummary,
    pub textures: Vec<RenderedTitleTexture>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TitleTextureSet {
    pub symbol: String,
    pub pairs: Vec<TitleTexturePair>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TitleTexturePair {
    pub image_load_index: usize,
    pub palette_slot: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TitlePatternRecord {
    pub index: usize,
    pub image_load_index: usize,
    pub source_x: u32,
    pub source_y: u32,
    pub width: u32,
    pub height: u32,
}

impl TitlePatternRecord {
    fn from_source(index: usize, record: TexturePatternRecord) -> Self {
        Self {
            index,
            image_load_index: record.image_load_index,
            source_x: record.source_x,
            source_y: record.source_y,
            width: record.width,
            height: record.height,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TitleTextureSummary {
    pub load_records: usize,
    pub image_loads: usize,
    pub palette_loads: usize,
    pub consumer_pairs: usize,
    pub unique_rendered_pairs: usize,
    pub texture_pixel_format: &'static str,
    pub palette_pixel_format: &'static str,
    pub palette_storage_mode: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderedTitleTexture {
    pub image_load_index: usize,
    pub palette_slot: usize,
    pub image_file_record: usize,
    pub palette_file_record: usize,
    pub width: u32,
    pub height: u32,
    pub decoded_trailing_byte_length: u64,
    pub swizzled_indices_sha256: String,
    pub linear_indices_sha256: String,
    pub uploaded_palette_sha256: String,
    pub rgba_sha256: String,
    pub png_path: String,
    pub png_sha256: String,
}

pub type TitleTextureInputs<'a> = TextureSourceInputs<'a>;

pub fn render_title_textures(
    inputs: TitleTextureInputs<'_>,
    output_directory: &Path,
) -> Result<TitleTextureCorpus> {
    let file = object::File::parse(inputs.executable).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips,
        "ELF architecture is not 32-bit MIPS"
    );
    ensure!(
        file.endianness() == Endianness::Little,
        "ELF is not little-endian"
    );

    let (_, load_table_bytes) = read_symbol_bytes(&file, TITLE_LOAD_TABLE_SYMBOL)?;
    let load_records = parse_load_records(&load_table_bytes)?;
    let image_load_count = load_records
        .iter()
        .take_while(|record| record.operation == 1)
        .count();
    ensure!(
        image_load_count > 0,
        "title load table has no decoded images"
    );
    ensure!(
        load_records[..image_load_count]
            .iter()
            .all(|record| record.operation == 1),
        "title image loads are not contiguous"
    );
    ensure!(
        load_records[image_load_count..]
            .iter()
            .all(|record| record.operation == 2),
        "title palette loads are not contiguous after image loads"
    );
    let palette_load_count = load_records.len() - image_load_count;
    ensure!(palette_load_count > 0, "title load table has no palettes");
    ensure!(
        load_records[image_load_count].destination == 0x1a40,
        "first title palette does not establish the expected GS base"
    );
    ensure!(
        load_records[image_load_count + 1..]
            .iter()
            .all(|record| record.destination == 0),
        "later title palettes unexpectedly override their GS destination"
    );

    let mut texture_sets = Vec::with_capacity(TITLE_TEXTURE_SET_SYMBOLS.len());
    let mut unique_pairs = BTreeSet::new();
    let mut pair_count = 0_usize;
    for symbol in TITLE_TEXTURE_SET_SYMBOLS {
        let (_, bytes) = read_symbol_bytes(&file, symbol)?;
        let pairs: Vec<_> = parse_texture_registration_pairs(symbol, &bytes)?
            .into_iter()
            .map(|pair| TitleTexturePair {
                image_load_index: pair.image_load_index,
                palette_slot: pair.palette_slot,
            })
            .collect();
        for pair in &pairs {
            ensure!(
                pair.image_load_index < image_load_count,
                "{symbol} references a non-image load: {}",
                pair.image_load_index
            );
            ensure!(
                pair.palette_slot < palette_load_count,
                "{symbol} references an unavailable palette slot: {}",
                pair.palette_slot
            );
            unique_pairs.insert(*pair);
        }
        pair_count += pairs.len();
        texture_sets.push(TitleTextureSet {
            symbol: symbol.to_string(),
            pairs,
        });
    }
    ensure!(
        unique_pairs.len() == image_load_count,
        "title consumers do not cover each loaded image exactly once as a unique pair"
    );
    let palette_by_image: BTreeMap<usize, usize> = unique_pairs
        .iter()
        .map(|pair| (pair.image_load_index, pair.palette_slot))
        .collect();
    ensure!(
        palette_by_image.len() == unique_pairs.len(),
        "a title image is paired with more than one palette"
    );

    let (_, file_list_bytes) = read_symbol_bytes(&file, IMAGE_FILE_LIST_SYMBOL)?;
    let file_records = parse_file_records(&file_list_bytes)?;
    let (_, pattern_bytes) = read_symbol_bytes(&file, TITLE_PATTERN_SYMBOL)?;
    let source_patterns = parse_pattern_records(TITLE_PATTERN_SYMBOL, &pattern_bytes)?;
    let patterns = source_patterns
        .into_iter()
        .enumerate()
        .map(|(index, record)| TitlePatternRecord::from_source(index, record))
        .collect();
    fs::create_dir_all(output_directory).with_context(|| {
        format!(
            "failed to create title texture output directory {}",
            output_directory.display()
        )
    })?;

    let mut textures = Vec::with_capacity(unique_pairs.len());
    for pair in unique_pairs {
        let image_load = load_records[pair.image_load_index];
        let palette_load = load_records[image_load_count + pair.palette_slot];
        let image_record = *file_records.get(image_load.file_record).with_context(|| {
            format!(
                "title image file record is out of range: {}",
                image_load.file_record
            )
        })?;
        let palette_record = *file_records
            .get(palette_load.file_record)
            .with_context(|| {
                format!(
                    "title palette file record is out of range: {}",
                    palette_load.file_record
                )
            })?;
        let output_path = output_directory.join(format!(
            "title-texture-{:02}-palette-{:02}.png",
            pair.image_load_index, pair.palette_slot
        ));
        let rendered = render_indexed_texture(
            inputs.image_container,
            image_record,
            palette_record,
            palette_load.flags & 2 != 0,
            &output_path,
        )?;
        textures.push(RenderedTitleTexture {
            image_load_index: pair.image_load_index,
            palette_slot: pair.palette_slot,
            image_file_record: image_load.file_record,
            palette_file_record: palette_load.file_record,
            width: rendered.width,
            height: rendered.height,
            decoded_trailing_byte_length: rendered.decoded_trailing_byte_length,
            swizzled_indices_sha256: rendered.swizzled_indices_sha256,
            linear_indices_sha256: rendered.linear_indices_sha256,
            uploaded_palette_sha256: rendered.uploaded_palette_sha256,
            rgba_sha256: rendered.rgba_sha256,
            png_path: rendered.png_path,
            png_sha256: rendered.png_sha256,
        });
    }

    Ok(TitleTextureCorpus {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        image_container_iso_path: inputs.image_container_iso_path,
        image_container_sha256: inputs.image_container_sha256,
        load_table_symbol: TITLE_LOAD_TABLE_SYMBOL.to_string(),
        file_list_symbol: IMAGE_FILE_LIST_SYMBOL.to_string(),
        texture_sets,
        pattern_symbol: TITLE_PATTERN_SYMBOL.to_string(),
        patterns,
        summary: TitleTextureSummary {
            load_records: load_records.len(),
            image_loads: image_load_count,
            palette_loads: palette_load_count,
            consumer_pairs: pair_count,
            unique_rendered_pairs: textures.len(),
            texture_pixel_format: "PSMT8 (0x13)",
            palette_pixel_format: "PSMCT32 (0x00)",
            palette_storage_mode: "CSM1",
        },
        textures,
    })
}
