use std::path::Path;

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::Serialize;

use super::render::render_indexed_font_atlas;
use super::{
    FontAtlasInputs, FontGlyphSet, RenderedFontAtlas, TranslationScope, TypedInstructionEvidence,
};
use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::{parse_file_records, parse_load_records, sha256};

mod lookup;
mod registration;
mod renderer;
mod source;

use lookup::{parse_tutorial_glyphs, validate_lookup_path};
use registration::validate_registration_path;
use renderer::validate_renderer_path;
use source::validate_source_path;

const LOAD_FILE_ID: usize = 0;
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_D.BIN;1";
const FILE_LIST_SYMBOL: &str = "FileList_Image";
const LOAD_TABLE_SYMBOL: &str = "ExpTexTbl";
const REGISTRATION_SET_SYMBOL: &str = "ExpTexList";
const LOADER_SYMBOL: &str = "Exp_Demo_Init";
const REGISTRATION_OWNER_SYMBOL: &str = "Exp_Demo_Run";
const READ_FILE_SYMBOL: &str = "aqCdvd_ReadFile";
const REGISTRAR_SYMBOL: &str = "ActRegList_Regist3";
const REGISTRATION_BUFFER_SYMBOL: &str = "RegBuff";
const REGISTRATION_RECORD_BYTES: u64 = 0x90;
const FONT_REGISTRATION_SLOT: usize = 6;
const FONT_IMAGE_LOAD_INDEX: usize = 6;
const FONT_PALETTE_SLOT: usize = 6;
const REGISTRATION_PAIR_COUNT: usize = 8;
const FONT_IMAGE_FILE_RECORD: usize = 22;
const FONT_PALETTE_FILE_RECORD: usize = 82;
const RENDERER_SYMBOL: &str = "ExpMsgPrint_Act";
const LOOKUP_SYMBOL: &str = "GetFontParam_Exp";
const FONT_LIST_SYMBOL: &str = "FontList_Exp";
const SPRITE_PACKET_SYMBOL: &str = "aqSetSpritePacket";
const FULLWIDTH_COLUMNS: u32 = 16;

#[derive(Clone, Debug, Serialize)]
pub struct TutorialFontAtlasReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub image_container_iso_path: String,
    pub image_container_sha256: String,
    pub source: TutorialFontSourceEvidence,
    pub registration: TutorialFontRegistrationEvidence,
    pub lookup: TutorialFontLookupEvidence,
    pub renderer: TutorialFontRendererEvidence,
    pub atlas: RenderedFontAtlas,
    pub fullwidth: FontGlyphSet,
    pub translation_scope: TranslationScope,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialFontSourceEvidence {
    pub load_file_id: usize,
    pub mapped_iso_path: String,
    pub file_list_symbol: String,
    pub load_table_symbol: String,
    pub loader_symbol: String,
    pub read_file_symbol: String,
    pub image_loads: usize,
    pub palette_loads: usize,
    pub auxiliary_loads: Vec<TutorialAuxiliaryLoadEvidence>,
    pub instructions: Vec<TypedInstructionEvidence>,
    pub functions: Vec<TutorialTypedFunctionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialAuxiliaryLoadEvidence {
    pub flags: u32,
    pub operation: u32,
    pub file_record: usize,
    pub destination: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialTypedFunctionEvidence {
    pub symbol: String,
    pub instruction_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialFontRegistrationEvidence {
    pub registrar_symbol: String,
    pub registration_owner_symbol: String,
    pub registration_set_symbol: String,
    pub registration_buffer_symbol: String,
    pub record_byte_length: u64,
    pub font_registration_slot: usize,
    pub font_image_load_index: usize,
    pub font_palette_slot: usize,
    pub pairs: Vec<TutorialTextureRegistrationPair>,
    pub call_instructions: Vec<TypedInstructionEvidence>,
    pub buffer_layout_instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TutorialTextureRegistrationPair {
    pub image_load_index: usize,
    pub palette_slot: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialFontLookupEvidence {
    pub symbol: String,
    pub font_list_symbol: String,
    pub slots: TutorialLookupSlotSummary,
    pub missing_fallback_index: usize,
    pub instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialLookupSlotSummary {
    pub slots: usize,
    pub unique_codes: usize,
    pub shadowed_duplicate_slots: usize,
    pub duplicates: Vec<TutorialDuplicateCode>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialDuplicateCode {
    pub source_hex: String,
    pub first_slot: usize,
    pub shadowed_slots: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialFontRendererEvidence {
    pub symbol: String,
    pub sprite_packet_symbol: String,
    pub registration_buffer_address: String,
    pub selected_record_address: String,
    pub fullwidth_cell: String,
    pub instructions: Vec<TypedInstructionEvidence>,
}

pub fn render_tutorial_font_atlas(
    inputs: FontAtlasInputs<'_>,
    output_path: &Path,
) -> Result<TutorialFontAtlasReport> {
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
        "tutorial font consumer requires {IMAGE_CONTAINER_ISO_PATH}"
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

    let mut source = validate_source_path(&file, inputs.executable)?;
    let registration = validate_registration_path(&file, inputs.executable)?;
    let lookup = validate_lookup_path(&file)?;
    let renderer = validate_renderer_path(&file)?;

    let (_, load_table_bytes) = read_symbol_bytes(&file, LOAD_TABLE_SYMBOL)?;
    let load_records = parse_load_records(&load_table_bytes)?;
    let image_load_count = load_records
        .iter()
        .take_while(|record| record.operation == 1)
        .count();
    ensure!(
        image_load_count == 7,
        "tutorial image-load population changed"
    );
    let palette_load_count = load_records[image_load_count..]
        .iter()
        .take_while(|record| record.operation == 2)
        .count();
    ensure!(
        palette_load_count == 7,
        "tutorial palette-load population changed"
    );
    let auxiliary_loads = &load_records[image_load_count + palette_load_count..];
    ensure!(
        auxiliary_loads.len() == 1
            && auxiliary_loads[0].flags == 1
            && auxiliary_loads[0].operation == 0
            && auxiliary_loads[0].file_record == 73
            && auxiliary_loads[0].destination == 0x00b0_b9c0,
        "tutorial auxiliary texture load changed"
    );
    source.image_loads = image_load_count;
    source.palette_loads = palette_load_count;
    source.auxiliary_loads = auxiliary_loads
        .iter()
        .map(|record| TutorialAuxiliaryLoadEvidence {
            flags: record.flags,
            operation: record.operation,
            file_record: record.file_record,
            destination: format!("{:#010x}", record.destination),
        })
        .collect();

    let image_load = load_records[FONT_IMAGE_LOAD_INDEX];
    let palette_load = load_records[image_load_count + FONT_PALETTE_SLOT];
    ensure!(
        image_load.operation == 1
            && image_load.file_record == FONT_IMAGE_FILE_RECORD
            && palette_load.operation == 2
            && palette_load.file_record == FONT_PALETTE_FILE_RECORD,
        "tutorial font file-record pair changed"
    );
    let (_, file_list_bytes) = read_symbol_bytes(&file, FILE_LIST_SYMBOL)?;
    let file_records = parse_file_records(&file_list_bytes)?;
    let image_record = *file_records
        .get(image_load.file_record)
        .context("tutorial font image record is out of range")?;
    let palette_record = *file_records
        .get(palette_load.file_record)
        .context("tutorial font palette record is out of range")?;
    ensure!(
        image_record.width == 512 && image_record.height == 256,
        "tutorial font atlas dimensions changed"
    );
    let atlas = render_indexed_font_atlas(
        inputs.image_container,
        image_record,
        palette_record,
        palette_load.flags & 2 != 0,
        output_path,
        image_load_count,
        palette_load_count,
        FONT_IMAGE_LOAD_INDEX,
        FONT_PALETTE_SLOT,
        image_load.file_record,
        palette_load.file_record,
    )?;
    let (_, font_list_bytes) = read_symbol_bytes(&file, FONT_LIST_SYMBOL)?;
    let glyphs = parse_tutorial_glyphs(&font_list_bytes, atlas.width, atlas.height)?;

    Ok(TutorialFontAtlasReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        image_container_iso_path: inputs.image_container_iso_path,
        image_container_sha256: inputs.image_container_sha256,
        source,
        registration,
        lookup,
        renderer,
        atlas,
        fullwidth: FontGlyphSet {
            symbol: FONT_LIST_SYMBOL.to_string(),
            source_encoding: "Shift-JIS",
            role: "Japanese tutorial glyphs targeted for Korean replacement planning",
            glyphs,
        },
        translation_scope: TranslationScope {
            target: "Japanese tutorial text only",
            preserve: "English text and every asset outside the Japanese tutorial consumer path",
        },
        evidence_boundary: "static original-asset consumer linkage and local rendering only; no Korean allocation, insertion, or cold-boot display is proven",
    })
}

#[cfg(test)]
#[path = "tutorial/tutorial_tests.rs"]
mod tutorial_tests;
