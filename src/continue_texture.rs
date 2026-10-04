use std::path::Path;

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::Serialize;

use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::{
    LoadRecord, parse_file_records, parse_load_records, render_indexed_texture, sha256,
};
use crate::texture_pattern::{
    TexturePatternRecord, parse_pattern_records, validate_pattern_indices,
    validate_pattern_rectangles,
};
use crate::texture_source::TextureSourceInputs;
use crate::typed_evidence::TypedInstructionEvidence;

mod source;

use source::validate_continue_texture_source;

const LOAD_FILE_ID: usize = 1;
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_G.BIN;1";
const FILE_LIST_SYMBOL: &str = "FileList_Image2";
const LOAD_TABLE_SYMBOL: &str = "ContTexTbl";
const SCREEN_PATTERN_SYMBOL: &str = "ContPtn";
const NUMBER_PATTERN_SYMBOL: &str = "ContPtnTbl";
const LOADER_SYMBOL: &str = "Continue_Init";
const REGISTRATION_OWNER_SYMBOL: &str = "Continue_Run";
const SCREEN_PATTERN_OWNER_SYMBOL: &str = "Hito_Continue_Start";
const NUMBER_PATTERN_OWNER_SYMBOL: &str = "act_puyo_num";
const READ_FILE_SYMBOL: &str = "aqCdvd_ReadFile";
const REGISTRAR_SYMBOL: &str = "ActRegList_Regist2";
const PATTERN_CONSUMER_SYMBOL: &str = "Act_SetPtn";
const EXPECTED_LOAD_RECORDS: [(u32, u32, usize, u32); 6] = [
    (1, 1, 90, 0xb0c9c0),
    (0, 1, 91, 0),
    (0, 1, 11, 0),
    (3, 2, 213, 0x1a40),
    (2, 2, 214, 0),
    (2, 2, 134, 0),
];
const EXPECTED_SCREEN_PATTERN_INDICES: [usize; 14] = [0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0];

#[derive(Clone, Debug, Serialize)]
pub struct ContinueTextureCorpus {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub image_container_iso_path: String,
    pub image_container_sha256: String,
    pub source: ContinueTextureSourceEvidence,
    pub patterns: Vec<ContinuePatternSet>,
    pub summary: ContinueTextureSummary,
    pub textures: Vec<RenderedContinueTexture>,
    pub translation_scope: ContinueTextureTranslationScope,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContinueTextureSourceEvidence {
    pub load_file_id: usize,
    pub mapped_iso_path: String,
    pub file_list_symbol: String,
    pub load_table_symbol: String,
    pub loader_symbol: String,
    pub read_file_symbol: String,
    pub registration_owner_symbol: String,
    pub registrar_symbol: String,
    pub pattern_consumer_symbol: String,
    pub instructions: Vec<TypedInstructionEvidence>,
    pub registration_instructions: Vec<TypedInstructionEvidence>,
    pub screen_pattern_instructions: Vec<TypedInstructionEvidence>,
    pub number_pattern_instructions: Vec<TypedInstructionEvidence>,
    pub pattern_buffer_layout_instructions: Vec<TypedInstructionEvidence>,
    pub functions: Vec<ContinueTextureFunctionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContinueTextureFunctionEvidence {
    pub symbol: String,
    pub instruction_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContinuePatternSet {
    pub symbol: String,
    pub owner_symbol: String,
    pub records: Vec<ContinuePatternRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ContinuePatternRecord {
    pub image_load_index: usize,
    pub source_x: u32,
    pub source_y: u32,
    pub width: u32,
    pub height: u32,
}

impl From<TexturePatternRecord> for ContinuePatternRecord {
    fn from(record: TexturePatternRecord) -> Self {
        Self {
            image_load_index: record.image_load_index,
            source_x: record.source_x,
            source_y: record.source_y,
            width: record.width,
            height: record.height,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ContinueTextureSummary {
    pub load_records: usize,
    pub image_loads: usize,
    pub palette_loads: usize,
    pub pattern_records: usize,
    pub covered_image_loads: usize,
    pub texture_pixel_format: &'static str,
    pub palette_pixel_format: &'static str,
    pub palette_storage_mode: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderedContinueTexture {
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

#[derive(Clone, Debug, Serialize)]
pub struct ContinueTextureTranslationScope {
    pub target: &'static str,
    pub preserve: &'static str,
    pub review: &'static str,
}

pub fn render_continue_textures(
    inputs: TextureSourceInputs<'_>,
    output_directory: &Path,
) -> Result<ContinueTextureCorpus> {
    ensure!(
        sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    ensure!(
        sha256(inputs.image_container) == inputs.image_container_sha256,
        "image container bytes differ from the verified inventory hash"
    );
    ensure!(
        inputs.image_container_iso_path == IMAGE_CONTAINER_ISO_PATH,
        "continue texture consumer requires {IMAGE_CONTAINER_ISO_PATH}"
    );
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

    let source = validate_continue_texture_source(&file, inputs.executable)?;
    let (_, load_table_bytes) = read_symbol_bytes(&file, LOAD_TABLE_SYMBOL)?;
    let load_records = parse_load_records(&load_table_bytes)?;
    let image_load_count = validate_continue_load_records(&load_records)?;
    let palette_load_count = load_records.len() - image_load_count;

    let (_, screen_pattern_bytes) = read_symbol_bytes(&file, SCREEN_PATTERN_SYMBOL)?;
    let screen_pattern_records =
        parse_pattern_records(SCREEN_PATTERN_SYMBOL, &screen_pattern_bytes)?;
    validate_pattern_indices(
        SCREEN_PATTERN_SYMBOL,
        &screen_pattern_records,
        &EXPECTED_SCREEN_PATTERN_INDICES,
    )?;
    let (_, number_pattern_bytes) = read_symbol_bytes(&file, NUMBER_PATTERN_SYMBOL)?;
    let number_pattern_records =
        parse_pattern_records(NUMBER_PATTERN_SYMBOL, &number_pattern_bytes)?;
    let expected_number_indices = vec![2; 35];
    validate_pattern_indices(
        NUMBER_PATTERN_SYMBOL,
        &number_pattern_records,
        &expected_number_indices,
    )?;

    let (_, file_list_bytes) = read_symbol_bytes(&file, FILE_LIST_SYMBOL)?;
    let file_records = parse_file_records(&file_list_bytes)?;
    let mut textures = Vec::with_capacity(image_load_count);
    for image_load_index in 0..image_load_count {
        let image_load = load_records[image_load_index];
        let palette_load = load_records[image_load_count + image_load_index];
        let image_record = *file_records
            .get(image_load.file_record)
            .context("continue image file record is out of range")?;
        let palette_record = *file_records
            .get(palette_load.file_record)
            .context("continue palette file record is out of range")?;
        validate_pattern_rectangles(
            "continue pattern",
            image_load_index,
            image_record.width,
            image_record.height,
            screen_pattern_records.iter().chain(&number_pattern_records),
        )?;
        let output_path = output_directory.join(format!(
            "continue-texture-{image_load_index:02}-palette-{image_load_index:02}.png"
        ));
        let rendered = render_indexed_texture(
            inputs.image_container,
            image_record,
            palette_record,
            palette_load.flags & 2 != 0,
            &output_path,
        )?;
        textures.push(RenderedContinueTexture {
            image_load_index,
            palette_slot: image_load_index,
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

    let pattern_record_count = screen_pattern_records.len() + number_pattern_records.len();
    Ok(ContinueTextureCorpus {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        image_container_iso_path: inputs.image_container_iso_path,
        image_container_sha256: inputs.image_container_sha256,
        source,
        patterns: vec![
            ContinuePatternSet {
                symbol: SCREEN_PATTERN_SYMBOL.to_string(),
                owner_symbol: SCREEN_PATTERN_OWNER_SYMBOL.to_string(),
                records: screen_pattern_records
                    .into_iter()
                    .map(ContinuePatternRecord::from)
                    .collect(),
            },
            ContinuePatternSet {
                symbol: NUMBER_PATTERN_SYMBOL.to_string(),
                owner_symbol: NUMBER_PATTERN_OWNER_SYMBOL.to_string(),
                records: number_pattern_records
                    .into_iter()
                    .map(ContinuePatternRecord::from)
                    .collect(),
            },
        ],
        summary: ContinueTextureSummary {
            load_records: load_records.len(),
            image_loads: image_load_count,
            palette_loads: palette_load_count,
            pattern_records: pattern_record_count,
            covered_image_loads: textures.len(),
            texture_pixel_format: "PSMT8 (0x13)",
            palette_pixel_format: "PSMCT32 (0x00)",
            palette_storage_mode: "CSM1",
        },
        textures,
        translation_scope: ContinueTextureTranslationScope {
            target: "Japanese text baked into continue-screen textures only",
            preserve: "English text and every pixel outside approved Japanese text regions",
            review: "rendered texture classification requires human visual review before any edit",
        },
        evidence_boundary: "static original-asset load, registration, pattern linkage, and local rendering only; no edit or cold-boot screen display is proven",
    })
}

fn validate_continue_load_records(records: &[LoadRecord]) -> Result<usize> {
    let actual = records
        .iter()
        .map(|record| {
            (
                record.flags,
                record.operation,
                record.file_record,
                record.destination,
            )
        })
        .collect::<Vec<_>>();
    ensure!(
        actual == EXPECTED_LOAD_RECORDS,
        "continue load records changed"
    );
    Ok(3)
}

#[cfg(test)]
#[path = "continue_texture/continue_texture_tests.rs"]
mod continue_texture_tests;
