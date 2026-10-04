use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::Serialize;

use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::{
    LoadRecord, TextureRegistrationPair, parse_file_records, parse_load_records,
    parse_texture_registration_pairs, render_indexed_texture, sha256,
};
use crate::texture_pattern::{TexturePatternRecord, parse_pattern_records};
use crate::texture_source::TextureSourceInputs;
use crate::typed_evidence::TypedInstructionEvidence;

mod source;

use source::validate_rank_texture_source;

const LOAD_FILE_ID: usize = 1;
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_G.BIN;1";
const FILE_LIST_SYMBOL: &str = "FileList_Image2";
const LOAD_TABLE_SYMBOL: &str = "RankTexTbl";
const REGISTRATION_SET_SYMBOL: &str = "RankTexList";
const PATTERN_TABLE_SYMBOL: &str = "RankPtn";
const LOADER_SYMBOL: &str = "RankDisp_Init";
const REGISTRATION_OWNER_SYMBOL: &str = "RankDisp_Run";
const READ_FILE_SYMBOL: &str = "aqCdvd_ReadFile";
const REGISTRAR_SYMBOL: &str = "ActRegList_Regist3";
const PATTERN_CONSUMER_SYMBOL: &str = "Make_DispOnly";
const EXPECTED_LOAD_RECORDS: [(u32, u32, usize, u32); 12] = [
    (1, 1, 119, 0xb0c9c0),
    (0, 1, 120, 0),
    (0, 1, 121, 0),
    (0, 1, 12, 0),
    (0, 1, 118, 0),
    (0, 1, 122, 0),
    (3, 2, 239, 0x1a40),
    (2, 2, 240, 0),
    (2, 2, 241, 0),
    (2, 2, 135, 0),
    (2, 2, 238, 0),
    (2, 2, 242, 0),
];
const EXPECTED_PAIRS: [(usize, usize); 7] =
    [(0, 0), (5, 5), (2, 2), (3, 3), (1, 1), (2, 2), (4, 4)];

#[derive(Clone, Debug, Serialize)]
pub struct RankTextureCorpus {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub image_container_iso_path: String,
    pub image_container_sha256: String,
    pub source: RankTextureSourceEvidence,
    pub registration: RankTextureRegistrationEvidence,
    pub patterns: RankPatternSet,
    pub summary: RankTextureSummary,
    pub textures: Vec<RenderedRankTexture>,
    pub translation_scope: RankTextureTranslationScope,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RankTextureSourceEvidence {
    pub load_file_id: usize,
    pub mapped_iso_path: String,
    pub file_list_symbol: String,
    pub load_table_symbol: String,
    pub loader_symbol: String,
    pub read_file_symbol: String,
    pub instructions: Vec<TypedInstructionEvidence>,
    pub functions: Vec<RankTextureFunctionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RankTextureFunctionEvidence {
    pub symbol: String,
    pub instruction_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct RankTextureRegistrationEvidence {
    pub owner_symbol: String,
    pub registrar_symbol: String,
    pub registration_set_symbol: String,
    pub pattern_table_symbol: String,
    pub pattern_consumer_symbol: String,
    pub registration_record_bytes: u64,
    pub pairs: Vec<RankTexturePair>,
    pub call_instructions: Vec<TypedInstructionEvidence>,
    pub pattern_instructions: Vec<TypedInstructionEvidence>,
    pub buffer_layout_instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RankTexturePair {
    pub image_load_index: usize,
    pub palette_slot: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct RankPatternSet {
    pub symbol: String,
    pub consumer_symbol: String,
    pub records: Vec<RankPatternRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RankPatternRecord {
    pub index: usize,
    pub registration_slot: usize,
    pub image_load_index: usize,
    pub palette_slot: usize,
    pub source_x: u32,
    pub source_y: u32,
    pub width: u32,
    pub height: u32,
}

impl RankPatternRecord {
    fn from_source(
        index: usize,
        record: TexturePatternRecord,
        pair: TextureRegistrationPair,
    ) -> Self {
        Self {
            index,
            registration_slot: record.image_load_index,
            image_load_index: pair.image_load_index,
            palette_slot: pair.palette_slot,
            source_x: record.source_x,
            source_y: record.source_y,
            width: record.width,
            height: record.height,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RankTextureSummary {
    pub load_records: usize,
    pub image_loads: usize,
    pub palette_loads: usize,
    pub consumer_pairs: usize,
    pub unique_rendered_pairs: usize,
    pub repeated_consumer_pairs: usize,
    pub pattern_records: usize,
    pub texture_pixel_format: &'static str,
    pub palette_pixel_format: &'static str,
    pub palette_storage_mode: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderedRankTexture {
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
pub struct RankTextureTranslationScope {
    pub target: &'static str,
    pub preserve: &'static str,
    pub review: &'static str,
}

pub type RankTextureInputs<'a> = TextureSourceInputs<'a>;

pub fn render_rank_textures(
    inputs: RankTextureInputs<'_>,
    output_directory: &Path,
) -> Result<RankTextureCorpus> {
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
        "ranking texture consumer requires {IMAGE_CONTAINER_ISO_PATH}"
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

    let (source, call_instructions, pattern_instructions, buffer_layout_instructions) =
        validate_rank_texture_source(&file, inputs.executable)?;
    let (_, load_table_bytes) = read_symbol_bytes(&file, LOAD_TABLE_SYMBOL)?;
    let load_records = parse_load_records(&load_table_bytes)?;
    let image_load_count = validate_rank_load_records(&load_records)?;
    let palette_load_count = load_records.len() - image_load_count;

    let (_, registration_bytes) = read_symbol_bytes(&file, REGISTRATION_SET_SYMBOL)?;
    let parsed_pairs =
        parse_texture_registration_pairs(REGISTRATION_SET_SYMBOL, &registration_bytes)?;
    let unique_pairs = validate_rank_pairs(&parsed_pairs)?;
    let pairs = parsed_pairs
        .iter()
        .map(|pair| RankTexturePair {
            image_load_index: pair.image_load_index,
            palette_slot: pair.palette_slot,
        })
        .collect::<Vec<_>>();
    let (_, file_list_bytes) = read_symbol_bytes(&file, FILE_LIST_SYMBOL)?;
    let file_records = parse_file_records(&file_list_bytes)?;
    let (_, pattern_bytes) = read_symbol_bytes(&file, PATTERN_TABLE_SYMBOL)?;
    let source_patterns = parse_pattern_records(PATTERN_TABLE_SYMBOL, &pattern_bytes)?;
    ensure!(
        !source_patterns.is_empty(),
        "ranking pattern table is empty"
    );
    validate_rank_patterns(
        &source_patterns,
        &parsed_pairs,
        &load_records,
        &file_records,
    )?;
    let mut textures = Vec::with_capacity(unique_pairs.len());
    for pair in unique_pairs {
        let image_load = load_records[pair.image_load_index];
        let palette_load = load_records[image_load_count + pair.palette_slot];
        let image_record = *file_records
            .get(image_load.file_record)
            .context("ranking image file record is out of range")?;
        let palette_record = *file_records
            .get(palette_load.file_record)
            .context("ranking palette file record is out of range")?;
        let output_path = output_directory.join(format!(
            "rank-texture-{:02}-palette-{:02}.png",
            pair.image_load_index, pair.palette_slot
        ));
        let rendered = render_indexed_texture(
            inputs.image_container,
            image_record,
            palette_record,
            palette_load.flags & 2 != 0,
            &output_path,
        )?;
        textures.push(RenderedRankTexture {
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

    Ok(RankTextureCorpus {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        image_container_iso_path: inputs.image_container_iso_path,
        image_container_sha256: inputs.image_container_sha256,
        source,
        registration: RankTextureRegistrationEvidence {
            owner_symbol: REGISTRATION_OWNER_SYMBOL.to_string(),
            registrar_symbol: REGISTRAR_SYMBOL.to_string(),
            registration_set_symbol: REGISTRATION_SET_SYMBOL.to_string(),
            pattern_table_symbol: PATTERN_TABLE_SYMBOL.to_string(),
            pattern_consumer_symbol: PATTERN_CONSUMER_SYMBOL.to_string(),
            registration_record_bytes: 0x90,
            pairs,
            call_instructions,
            pattern_instructions,
            buffer_layout_instructions,
        },
        patterns: RankPatternSet {
            symbol: PATTERN_TABLE_SYMBOL.to_string(),
            consumer_symbol: PATTERN_CONSUMER_SYMBOL.to_string(),
            records: source_patterns
                .iter()
                .copied()
                .enumerate()
                .map(|(index, record)| {
                    RankPatternRecord::from_source(
                        index,
                        record,
                        parsed_pairs[record.image_load_index],
                    )
                })
                .collect(),
        },
        summary: RankTextureSummary {
            load_records: load_records.len(),
            image_loads: image_load_count,
            palette_loads: palette_load_count,
            consumer_pairs: parsed_pairs.len(),
            unique_rendered_pairs: textures.len(),
            repeated_consumer_pairs: parsed_pairs.len() - textures.len(),
            pattern_records: source_patterns.len(),
            texture_pixel_format: "PSMT8 (0x13)",
            palette_pixel_format: "PSMCT32 (0x00)",
            palette_storage_mode: "CSM1",
        },
        textures,
        translation_scope: RankTextureTranslationScope {
            target: "Japanese text baked into ranking-screen textures only",
            preserve: "English text and every pixel outside approved Japanese text regions",
            review: "rendered texture classification requires human visual review before any edit",
        },
        evidence_boundary: "static original-asset load, registration, pattern linkage, and local rendering only; no edit or cold-boot screen display is proven",
    })
}

fn validate_rank_load_records(records: &[LoadRecord]) -> Result<usize> {
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
        "ranking load records changed"
    );
    Ok(6)
}

fn validate_rank_pairs(
    pairs: &[TextureRegistrationPair],
) -> Result<BTreeSet<TextureRegistrationPair>> {
    let actual = pairs
        .iter()
        .map(|pair| (pair.image_load_index, pair.palette_slot))
        .collect::<Vec<_>>();
    ensure!(
        actual == EXPECTED_PAIRS,
        "ranking registration pairs changed"
    );
    let unique = pairs.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        unique.len() == 6
            && unique
                .iter()
                .all(|pair| pair.image_load_index < 6 && pair.palette_slot < 6),
        "ranking registration pair coverage changed"
    );
    Ok(unique)
}

fn validate_rank_patterns(
    patterns: &[TexturePatternRecord],
    pairs: &[TextureRegistrationPair],
    loads: &[LoadRecord],
    file_records: &[crate::indexed_texture::FileRecord],
) -> Result<()> {
    for (index, pattern) in patterns.iter().enumerate() {
        let pair = pairs.get(pattern.image_load_index).with_context(|| {
            format!("ranking pattern {index} references an unavailable registration slot")
        })?;
        let load = loads.get(pair.image_load_index).with_context(|| {
            format!("ranking pattern {index} resolves to an unavailable image load")
        })?;
        let image = file_records.get(load.file_record).with_context(|| {
            format!("ranking pattern {index} resolves to an unavailable image file record")
        })?;
        let right = pattern
            .source_x
            .checked_add(pattern.width)
            .context("ranking pattern horizontal range overflow")?;
        let bottom = pattern
            .source_y
            .checked_add(pattern.height)
            .context("ranking pattern vertical range overflow")?;
        ensure!(
            pattern.width > 0
                && pattern.height > 0
                && right <= image.width
                && bottom <= image.height,
            "ranking pattern {index} exceeds resolved image load {}",
            pair.image_load_index
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "rank_texture/rank_texture_tests.rs"]
mod rank_texture_tests;
