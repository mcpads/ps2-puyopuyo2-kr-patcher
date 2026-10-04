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

use source::validate_rule_texture_source;

const LOAD_FILE_ID: usize = 0;
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_D.BIN;1";
const FILE_LIST_SYMBOL: &str = "FileList_Image";
const LOAD_TABLE_SYMBOL: &str = "RuleSel_TexTbl";
const SELECT_PATTERN_SYMBOL: &str = "RuleSel_Ptn";
const CHARACTER_PATTERN_SYMBOL: &str = "Rule_Kar_Ptn";
const LOADER_SYMBOL: &str = "Rule_Init";
const OWNER_SYMBOL: &str = "Rule_Run";
const READ_FILE_SYMBOL: &str = "aqCdvd_ReadFile";
const REGISTRAR_SYMBOL: &str = "ActRegList_Regist2";
const DISPLAY_CONSUMER_SYMBOL: &str = "Make_DispOnly";
const PATTERN_CONSUMER_SYMBOL: &str = "Act_SetPtn";
const CELL_CONSUMER_SYMBOL: &str = "Act_CellInit";
const EXPECTED_LOAD_RECORDS: [(u32, u32, usize, u32); 6] = [
    (1, 1, 46, 0xb0c9c0),
    (0, 1, 47, 0),
    (0, 1, 45, 0),
    (3, 2, 106, 0x1a40),
    (2, 2, 107, 0),
    (2, 2, 105, 0),
];
const EXPECTED_SELECT_PATTERN_INDICES: [usize; 16] =
    [0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];

#[derive(Clone, Debug, Serialize)]
pub struct RuleTextureCorpus {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub image_container_iso_path: String,
    pub image_container_sha256: String,
    pub source: RuleTextureSourceEvidence,
    pub patterns: Vec<RulePatternSet>,
    pub summary: RuleTextureSummary,
    pub textures: Vec<RenderedRuleTexture>,
    pub translation_scope: RuleTextureTranslationScope,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleTextureSourceEvidence {
    pub load_file_id: usize,
    pub mapped_iso_path: String,
    pub file_list_symbol: String,
    pub load_table_symbol: String,
    pub loader_symbol: String,
    pub owner_symbol: String,
    pub read_file_symbol: String,
    pub registrar_symbol: String,
    pub display_consumer_symbol: String,
    pub pattern_consumer_symbol: String,
    pub cell_consumer_symbol: String,
    pub load_instructions: Vec<TypedInstructionEvidence>,
    pub registration_instructions: Vec<TypedInstructionEvidence>,
    pub select_pattern_instructions: Vec<TypedInstructionEvidence>,
    pub character_pattern_instructions: Vec<TypedInstructionEvidence>,
    pub pattern_buffer_layout_instructions: Vec<TypedInstructionEvidence>,
    pub functions: Vec<RuleTextureFunctionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleTextureFunctionEvidence {
    pub symbol: String,
    pub instruction_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct RulePatternSet {
    pub symbol: String,
    pub owner_symbol: String,
    pub records: Vec<RulePatternRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RulePatternRecord {
    pub image_load_index: usize,
    pub source_x: u32,
    pub source_y: u32,
    pub width: u32,
    pub height: u32,
}

impl From<TexturePatternRecord> for RulePatternRecord {
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
pub struct RuleTextureSummary {
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
pub struct RenderedRuleTexture {
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
pub struct RuleTextureTranslationScope {
    pub target: &'static str,
    pub preserve: &'static str,
    pub review: &'static str,
}

pub fn render_rule_textures(
    inputs: TextureSourceInputs<'_>,
    output_directory: &Path,
) -> Result<RuleTextureCorpus> {
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
        "rule texture consumer requires {IMAGE_CONTAINER_ISO_PATH}"
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

    let source = validate_rule_texture_source(&file, inputs.executable)?;
    let (_, load_table_bytes) = read_symbol_bytes(&file, LOAD_TABLE_SYMBOL)?;
    let load_records = parse_load_records(&load_table_bytes)?;
    let image_load_count = validate_rule_load_records(&load_records)?;
    let palette_load_count = load_records.len() - image_load_count;

    let (_, select_pattern_bytes) = read_symbol_bytes(&file, SELECT_PATTERN_SYMBOL)?;
    let select_pattern_records =
        parse_pattern_records(SELECT_PATTERN_SYMBOL, &select_pattern_bytes)?;
    validate_pattern_indices(
        SELECT_PATTERN_SYMBOL,
        &select_pattern_records,
        &EXPECTED_SELECT_PATTERN_INDICES,
    )?;
    let (_, character_pattern_bytes) = read_symbol_bytes(&file, CHARACTER_PATTERN_SYMBOL)?;
    let character_pattern_records =
        parse_pattern_records(CHARACTER_PATTERN_SYMBOL, &character_pattern_bytes)?;
    let expected_character_indices = vec![2; 27];
    validate_pattern_indices(
        CHARACTER_PATTERN_SYMBOL,
        &character_pattern_records,
        &expected_character_indices,
    )?;

    let (_, file_list_bytes) = read_symbol_bytes(&file, FILE_LIST_SYMBOL)?;
    let file_records = parse_file_records(&file_list_bytes)?;
    let mut textures = Vec::with_capacity(image_load_count);
    for image_load_index in 0..image_load_count {
        let image_load = load_records[image_load_index];
        let palette_load = load_records[image_load_count + image_load_index];
        let image_record = *file_records
            .get(image_load.file_record)
            .context("rule image file record is out of range")?;
        let palette_record = *file_records
            .get(palette_load.file_record)
            .context("rule palette file record is out of range")?;
        validate_pattern_rectangles(
            "rule pattern",
            image_load_index,
            image_record.width,
            image_record.height,
            select_pattern_records
                .iter()
                .chain(&character_pattern_records),
        )?;
        let output_path = output_directory.join(format!(
            "rule-texture-{image_load_index:02}-palette-{image_load_index:02}.png"
        ));
        let rendered = render_indexed_texture(
            inputs.image_container,
            image_record,
            palette_record,
            palette_load.flags & 2 != 0,
            &output_path,
        )?;
        textures.push(RenderedRuleTexture {
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

    let pattern_record_count = select_pattern_records.len() + character_pattern_records.len();
    Ok(RuleTextureCorpus {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        image_container_iso_path: inputs.image_container_iso_path,
        image_container_sha256: inputs.image_container_sha256,
        source,
        patterns: vec![
            RulePatternSet {
                symbol: SELECT_PATTERN_SYMBOL.to_string(),
                owner_symbol: OWNER_SYMBOL.to_string(),
                records: select_pattern_records
                    .into_iter()
                    .map(RulePatternRecord::from)
                    .collect(),
            },
            RulePatternSet {
                symbol: CHARACTER_PATTERN_SYMBOL.to_string(),
                owner_symbol: OWNER_SYMBOL.to_string(),
                records: character_pattern_records
                    .into_iter()
                    .map(RulePatternRecord::from)
                    .collect(),
            },
        ],
        summary: RuleTextureSummary {
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
        translation_scope: RuleTextureTranslationScope {
            target: "Japanese text baked into rule-selection textures only",
            preserve: "English text and every pixel outside approved Japanese text regions",
            review: "rendered texture classification requires human visual review before any edit",
        },
        evidence_boundary: "static original-asset load, registration, pattern linkage, and local rendering only; no edit or cold-boot screen display is proven",
    })
}

fn validate_rule_load_records(records: &[LoadRecord]) -> Result<usize> {
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
    ensure!(actual == EXPECTED_LOAD_RECORDS, "rule load records changed");
    Ok(3)
}

#[cfg(test)]
#[path = "rule_texture/rule_texture_tests.rs"]
mod rule_texture_tests;
