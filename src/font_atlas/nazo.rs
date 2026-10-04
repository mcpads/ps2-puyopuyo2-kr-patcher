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

use lookup::{parse_nazo_fullwidth_glyphs, parse_nazo_halfwidth_glyphs, validate_lookup_path};
use registration::validate_registration_path;
use renderer::validate_renderer_path;
use source::validate_source_path;

const LOAD_FILE_ID: usize = 1;
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_G.BIN;1";
const FILE_LIST_SYMBOL: &str = "FileList_Image2";
const LOAD_TABLE_SYMBOL: &str = "NazoTexTbl";
const LOADER_SYMBOL: &str = "Nazo_Menu_Init";
const READ_FILE_SYMBOL: &str = "aqCdvd_ReadFile";
const REGISTRAR_SYMBOL: &str = "ActRegList_Regist3";
const REGISTRATION_OWNER_SYMBOLS: [&str; 3] = ["Nazo_Menu_Run", "Puyo_Nazo_Run", "Nazo_Edit_Run"];
const REGISTRATION_SET_SYMBOLS: [&str; 8] = [
    "Nazo_TexReg_Menu",
    "Nazo_TexReg_Nazo",
    "Nazo_TexReg_Game",
    "Nazo_TexReg_Edit_0",
    "Nazo_TexReg_Edit_1",
    "Nazo_TexReg_Edit_2",
    "Nazo_TexReg_Edit_4",
    "Nazo_TexReg_Present",
];
const RENDERER_SYMBOL: &str = "NazoMsgPrint_Act";
const LOOKUP_SYMBOL: &str = "GetFontParam_Nazo";
const FULLWIDTH_LIST_SYMBOL: &str = "FontList_Nazo";
const HALFWIDTH_LIST_SYMBOL: &str = "FontList_Nazo_Han";
const SPRITE_PACKET_SYMBOL: &str = "aqSetSpritePacket";
const REGISTRATION_BUFFER_SYMBOL: &str = "RegBuff";
const REGISTRATION_RECORD_BYTES: u64 = 0x90;
const FONT_REGISTRATION_SLOT: usize = 8;
const FONT_IMAGE_LOAD_INDEX: usize = 25;
const FONT_PALETTE_SLOT: usize = 7;
const FULLWIDTH_COLUMNS: u32 = 16;
const HALFWIDTH_COLUMNS: u32 = 32;
const HALFWIDTH_Y: u32 = 224;

#[derive(Clone, Debug, Serialize)]
pub struct NazoFontAtlasReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub image_container_iso_path: String,
    pub image_container_sha256: String,
    pub source: NazoFontSourceEvidence,
    pub registrations: NazoFontRegistrationEvidence,
    pub lookup: NazoFontLookupEvidence,
    pub renderer: NazoFontRendererEvidence,
    pub atlas: RenderedFontAtlas,
    pub fullwidth: FontGlyphSet,
    pub halfwidth: FontGlyphSet,
    pub translation_scope: TranslationScope,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoFontSourceEvidence {
    pub load_file_id: usize,
    pub mapped_iso_path: String,
    pub file_list_symbol: String,
    pub load_table_symbol: String,
    pub loader_symbol: String,
    pub read_file_symbol: String,
    pub instructions: Vec<TypedInstructionEvidence>,
    pub functions: Vec<TypedFunctionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TypedFunctionEvidence {
    pub symbol: String,
    pub instruction_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoFontRegistrationEvidence {
    pub registrar_symbol: String,
    pub registration_buffer_symbol: String,
    pub record_byte_length: u64,
    pub font_registration_slot: usize,
    pub font_image_load_index: usize,
    pub font_palette_slot: usize,
    pub registration_sets: Vec<NazoTextureRegistrationSet>,
    pub call_sites: Vec<NazoTextureRegistrationCall>,
    pub buffer_layout_instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoTextureRegistrationSet {
    pub symbol: String,
    pub pairs: Vec<NazoTextureRegistrationPair>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct NazoTextureRegistrationPair {
    pub image_load_index: usize,
    pub palette_slot: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoTextureRegistrationCall {
    pub owner_symbol: String,
    pub address: String,
    pub registration_set_symbol: String,
    pub instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoFontLookupEvidence {
    pub symbol: String,
    pub fullwidth_list_symbol: String,
    pub halfwidth_list_symbol: String,
    pub fullwidth_slots: NazoLookupSlotSummary,
    pub halfwidth_slots: NazoLookupSlotSummary,
    pub missing_fullwidth_fallback_index: usize,
    pub instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoLookupSlotSummary {
    pub slots: usize,
    pub unique_codes: usize,
    pub shadowed_duplicate_slots: usize,
    pub duplicates: Vec<NazoDuplicateCode>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoDuplicateCode {
    pub source_hex: String,
    pub first_slot: usize,
    pub shadowed_slots: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoFontRendererEvidence {
    pub symbol: String,
    pub sprite_packet_symbol: String,
    pub registration_buffer_address: String,
    pub selected_record_address: String,
    pub fullwidth_cell: String,
    pub halfwidth_cell: String,
    pub instructions: Vec<TypedInstructionEvidence>,
}

pub fn render_nazo_font_atlas(
    inputs: FontAtlasInputs<'_>,
    output_path: &Path,
) -> Result<NazoFontAtlasReport> {
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
        "Nazo font consumer requires {IMAGE_CONTAINER_ISO_PATH}"
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

    let source = validate_source_path(&file, inputs.executable)?;
    let registrations = validate_registration_path(&file, inputs.executable)?;
    let lookup = validate_lookup_path(&file)?;
    let renderer = validate_renderer_path(&file)?;

    let (_, load_table_bytes) = read_symbol_bytes(&file, LOAD_TABLE_SYMBOL)?;
    let load_records = parse_load_records(&load_table_bytes)?;
    let image_load_count = load_records
        .iter()
        .take_while(|record| record.operation == 1)
        .count();
    ensure!(image_load_count == 29, "Nazo image-load population changed");
    ensure!(
        load_records[..image_load_count]
            .iter()
            .all(|record| record.operation == 1),
        "Nazo image loads are not contiguous"
    );
    ensure!(
        load_records[image_load_count..]
            .iter()
            .all(|record| record.operation == 2),
        "Nazo palette loads are not contiguous after images"
    );
    let palette_load_count = load_records.len() - image_load_count;
    ensure!(
        palette_load_count == 16,
        "Nazo palette-load population changed"
    );
    let image_load = load_records[FONT_IMAGE_LOAD_INDEX];
    let palette_load = load_records[image_load_count + FONT_PALETTE_SLOT];
    ensure!(
        image_load.file_record == 113 && palette_load.file_record == 216,
        "Nazo font file-record pair changed"
    );

    let (_, file_list_bytes) = read_symbol_bytes(&file, FILE_LIST_SYMBOL)?;
    let file_records = parse_file_records(&file_list_bytes)?;
    let image_record = *file_records
        .get(image_load.file_record)
        .context("Nazo font image record is out of range")?;
    let palette_record = *file_records
        .get(palette_load.file_record)
        .context("Nazo font palette record is out of range")?;
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

    let (_, fullwidth_bytes) = read_symbol_bytes(&file, FULLWIDTH_LIST_SYMBOL)?;
    let (_, halfwidth_bytes) = read_symbol_bytes(&file, HALFWIDTH_LIST_SYMBOL)?;
    let fullwidth =
        parse_nazo_fullwidth_glyphs(&fullwidth_bytes, image_record.width, image_record.height)?;
    let halfwidth =
        parse_nazo_halfwidth_glyphs(&halfwidth_bytes, image_record.width, image_record.height)?;

    Ok(NazoFontAtlasReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        image_container_iso_path: inputs.image_container_iso_path,
        image_container_sha256: inputs.image_container_sha256,
        source,
        registrations,
        lookup,
        renderer,
        atlas,
        fullwidth: FontGlyphSet {
            symbol: FULLWIDTH_LIST_SYMBOL.to_string(),
            source_encoding: "Shift-JIS",
            role: "Japanese source glyphs targeted for Korean replacement planning",
            glyphs: fullwidth,
        },
        halfwidth: FontGlyphSet {
            symbol: HALFWIDTH_LIST_SYMBOL.to_string(),
            source_encoding: "ASCII subset",
            role: "Latin glyphs preserved for untranslated English text",
            glyphs: halfwidth,
        },
        translation_scope: TranslationScope {
            target: "Japanese text only",
            preserve: "English text and every halfwidth Latin glyph consumer",
        },
        evidence_boundary: "static original-asset consumer linkage and local rendering only; no Korean allocation, insertion, or cold-boot display is proven",
    })
}

#[cfg(test)]
#[path = "nazo/nazo_tests.rs"]
mod nazo_tests;
