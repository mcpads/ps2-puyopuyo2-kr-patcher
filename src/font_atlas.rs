use std::path::Path;

use anyhow::{Context, Result, ensure};
use encoding_rs::SHIFT_JIS;
use object::{Architecture, BinaryFormat, Endianness, Object};
use r5900::{GeneralRegister, InstructionForm, Operands};
use serde::Serialize;

use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::{parse_file_records, parse_load_records};
use crate::texture_source::TextureSourceInputs;
use crate::typed_evidence::{
    decode_instruction, instruction_evidence, validate_jump_target, validate_loaded_address,
};

pub use crate::typed_evidence::TypedInstructionEvidence;

pub mod nazo;
pub mod tutorial;

mod render;

use render::render_indexed_font_atlas;

const LOAD_TABLE_SYMBOL: &str = "MemLoadTexTbl";
const FILE_LIST_SYMBOL: &str = "FileList_Image";
const FULLWIDTH_LIST_SYMBOL: &str = "FontList_Zen";
const HALFWIDTH_LIST_SYMBOL: &str = "FontList_Han";
const SCREEN_OWNER_SYMBOL: &str = "Mem1stCheck_Run";
const SCREEN_ACTOR_SYMBOL: &str = "MemLoad_Act";
const MESSAGE_ACTOR_SYMBOL: &str = "MsgPrint_Act";
const FONT_CONSUMER_SYMBOL: &str = "FontPrint_Act";
const LOAD_TABLE_REGISTRAR_SYMBOL: &str = "ActRegList_Regist2";
const ACTOR_SETTER_SYMBOL: &str = "Act_SetBottom";
const PATTERN_SETTER_SYMBOL: &str = "Act_SetPtnDirect";
const FONT_TEXTURE_SLOT: usize = 1;
const FULLWIDTH_COLUMNS: u32 = 16;
const HALFWIDTH_COLUMNS: u32 = 32;

#[derive(Clone, Debug, Serialize)]
pub struct FontAtlasReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub image_container_iso_path: String,
    pub image_container_sha256: String,
    pub load_table_symbol: String,
    pub file_list_symbol: String,
    pub source_registration: FontSourceRegistrationEvidence,
    pub actor_links: Vec<ActorLinkEvidence>,
    pub consumer: FontConsumerEvidence,
    pub atlas: RenderedFontAtlas,
    pub fullwidth: FontGlyphSet,
    pub halfwidth: FontGlyphSet,
    pub translation_scope: TranslationScope,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontSourceRegistrationEvidence {
    pub owner_symbol: String,
    pub registrar_symbol: String,
    pub isa_profile: &'static str,
    pub instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActorLinkEvidence {
    pub owner_symbol: String,
    pub actor_symbol: String,
    pub setter_symbol: String,
    pub role: &'static str,
    pub instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontConsumerEvidence {
    pub symbol: String,
    pub isa_profile: &'static str,
    pub texture_slot: usize,
    pub slot_instruction_address: String,
    pub slot_instruction: String,
    pub pattern_setter_symbol: String,
    pub pattern_setter_calls: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderedFontAtlas {
    pub load_table_image_count: usize,
    pub load_table_palette_count: usize,
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
pub struct FontGlyphSet {
    pub symbol: String,
    pub source_encoding: &'static str,
    pub role: &'static str,
    pub glyphs: Vec<FontGlyph>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontGlyph {
    pub index: usize,
    pub source_hex: String,
    pub character: String,
    pub rectangle: GlyphRectangle,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct GlyphRectangle {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct TranslationScope {
    pub target: &'static str,
    pub preserve: &'static str,
}

pub type FontAtlasInputs<'a> = TextureSourceInputs<'a>;

pub fn render_font_atlas(
    inputs: FontAtlasInputs<'_>,
    output_path: &Path,
) -> Result<FontAtlasReport> {
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

    let source_registration = validate_source_registration(&file)?;
    let actor_links = validate_actor_links(&file)?;
    let consumer = validate_font_consumer(&file)?;
    let (_, load_table_bytes) = read_symbol_bytes(&file, LOAD_TABLE_SYMBOL)?;
    let load_records = parse_load_records(&load_table_bytes)?;
    let image_load_count = load_records
        .iter()
        .take_while(|record| record.operation == 1)
        .count();
    ensure!(
        image_load_count > FONT_TEXTURE_SLOT,
        "font texture slot has no image load"
    );
    ensure!(
        load_records[..image_load_count]
            .iter()
            .all(|record| record.operation == 1),
        "font-path image loads are not contiguous"
    );
    ensure!(
        load_records[image_load_count..]
            .iter()
            .all(|record| record.operation == 2),
        "font-path palette loads are not contiguous after images"
    );
    let palette_load_count = load_records.len() - image_load_count;
    ensure!(
        palette_load_count == image_load_count,
        "font-path image and palette counts differ"
    );

    let image_load = load_records[FONT_TEXTURE_SLOT];
    let palette_load = load_records[image_load_count + FONT_TEXTURE_SLOT];
    let (_, file_list_bytes) = read_symbol_bytes(&file, FILE_LIST_SYMBOL)?;
    let file_records = parse_file_records(&file_list_bytes)?;
    let image_record = *file_records
        .get(image_load.file_record)
        .context("font image file record is out of range")?;
    let palette_record = *file_records
        .get(palette_load.file_record)
        .context("font palette file record is out of range")?;
    let atlas = render_indexed_font_atlas(
        inputs.image_container,
        image_record,
        palette_record,
        palette_load.flags & 2 != 0,
        output_path,
        image_load_count,
        palette_load_count,
        FONT_TEXTURE_SLOT,
        FONT_TEXTURE_SLOT,
        image_load.file_record,
        palette_load.file_record,
    )?;

    let (_, fullwidth_bytes) = read_symbol_bytes(&file, FULLWIDTH_LIST_SYMBOL)?;
    let (_, halfwidth_bytes) = read_symbol_bytes(&file, HALFWIDTH_LIST_SYMBOL)?;
    let fullwidth =
        parse_fullwidth_glyphs(&fullwidth_bytes, image_record.width, image_record.height)?;
    let halfwidth =
        parse_halfwidth_glyphs(&halfwidth_bytes, image_record.width, image_record.height)?;

    Ok(FontAtlasReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        image_container_iso_path: inputs.image_container_iso_path,
        image_container_sha256: inputs.image_container_sha256,
        load_table_symbol: LOAD_TABLE_SYMBOL.to_string(),
        file_list_symbol: FILE_LIST_SYMBOL.to_string(),
        source_registration,
        actor_links,
        consumer,
        atlas,
        fullwidth: FontGlyphSet {
            symbol: FULLWIDTH_LIST_SYMBOL.to_string(),
            source_encoding: "Shift-JIS",
            role: "Japanese source and fullwidth glyphs targeted for Korean replacement planning",
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
            preserve: "English text, including Latin glyph consumers",
        },
    })
}

fn validate_source_registration(file: &object::File<'_>) -> Result<FontSourceRegistrationEvidence> {
    let (owner_address, owner_bytes) = read_symbol_bytes(file, SCREEN_OWNER_SYMBOL)?;
    let (load_table_address, _) = read_symbol_bytes(file, LOAD_TABLE_SYMBOL)?;
    let (registrar_address, _) = read_symbol_bytes(file, LOAD_TABLE_REGISTRAR_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        0x64,
        0x70,
        GeneralRegister::A1,
        load_table_address,
        "memory-card load table",
    )?;
    instructions.push(validate_jump_target(
        &owner_bytes,
        owner_address,
        0x6c,
        registrar_address,
        "memory-card load-table registration",
    )?);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(FontSourceRegistrationEvidence {
        owner_symbol: SCREEN_OWNER_SYMBOL.to_string(),
        registrar_symbol: LOAD_TABLE_REGISTRAR_SYMBOL.to_string(),
        isa_profile: r5900::PROFILE_ID,
        instructions,
    })
}

fn validate_actor_links(file: &object::File<'_>) -> Result<Vec<ActorLinkEvidence>> {
    Ok(vec![
        validate_actor_link(
            file,
            SCREEN_OWNER_SYMBOL,
            SCREEN_ACTOR_SYMBOL,
            0x80,
            0x88,
            0x90,
            "screen that consumes the registered memory-card texture table",
        )?,
        validate_actor_link(
            file,
            SCREEN_ACTOR_SYMBOL,
            MESSAGE_ACTOR_SYMBOL,
            0x1b4,
            0x1c0,
            0x1bc,
            "Japanese memory-card message renderer",
        )?,
        validate_actor_link(
            file,
            MESSAGE_ACTOR_SYMBOL,
            FONT_CONSUMER_SYMBOL,
            0x200,
            0x20c,
            0x208,
            "fullwidth Shift-JIS glyph renderer",
        )?,
        validate_actor_link(
            file,
            MESSAGE_ACTOR_SYMBOL,
            FONT_CONSUMER_SYMBOL,
            0x290,
            0x29c,
            0x298,
            "halfwidth ASCII glyph renderer preserved for English text",
        )?,
    ])
}

#[allow(clippy::too_many_arguments)]
fn validate_actor_link(
    file: &object::File<'_>,
    owner_symbol: &str,
    actor_symbol: &str,
    upper_offset: usize,
    lower_offset: usize,
    call_offset: usize,
    role: &'static str,
) -> Result<ActorLinkEvidence> {
    let (owner_address, owner_bytes) = read_symbol_bytes(file, owner_symbol)?;
    let (actor_address, _) = read_symbol_bytes(file, actor_symbol)?;
    let (setter_address, _) = read_symbol_bytes(file, ACTOR_SETTER_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        upper_offset,
        lower_offset,
        GeneralRegister::A0,
        actor_address,
        role,
    )?;
    instructions.push(validate_jump_target(
        &owner_bytes,
        owner_address,
        call_offset,
        setter_address,
        role,
    )?);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(ActorLinkEvidence {
        owner_symbol: owner_symbol.to_string(),
        actor_symbol: actor_symbol.to_string(),
        setter_symbol: ACTOR_SETTER_SYMBOL.to_string(),
        role,
        instructions,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_font_consumer(file: &object::File<'_>) -> Result<FontConsumerEvidence> {
    let (consumer_address, bytes) = read_symbol_bytes(file, FONT_CONSUMER_SYMBOL)?;
    let (setter_address, _) = read_symbol_bytes(file, PATTERN_SETTER_SYMBOL)?;
    let slot = decode_instruction(&bytes, consumer_address, 4)?;
    ensure!(
        slot.form() == InstructionForm::Addiu,
        "font slot setup is not addiu"
    );
    ensure!(
        matches!(
            slot.operands(),
            Operands::SignedImmediate {
                destination: GeneralRegister::A1,
                source: GeneralRegister::ZERO,
                immediate: 1,
            }
        ),
        "FontPrint_Act no longer selects RegBuff slot 1"
    );

    let mut calls = Vec::new();
    for offset in [0xc8_usize, 0x108] {
        let instruction = decode_instruction(&bytes, consumer_address, offset)?;
        ensure!(
            instruction.form() == InstructionForm::Jal,
            "font pattern call is not jal"
        );
        ensure!(
            matches!(instruction.operands(), Operands::Jump { target } if u64::from(*target) == setter_address),
            "FontPrint_Act pattern call no longer targets Act_SetPtnDirect"
        );
        calls.push(instruction_evidence(
            &bytes[offset..offset + 4],
            consumer_address + u64::try_from(offset)?,
            &instruction,
        ));
    }
    Ok(FontConsumerEvidence {
        symbol: FONT_CONSUMER_SYMBOL.to_string(),
        isa_profile: r5900::PROFILE_ID,
        texture_slot: FONT_TEXTURE_SLOT,
        slot_instruction_address: format!("{:#010x}", consumer_address + 4),
        slot_instruction: r5900::format_instruction(&slot),
        pattern_setter_symbol: PATTERN_SETTER_SYMBOL.to_string(),
        pattern_setter_calls: calls,
    })
}

fn parse_fullwidth_glyphs(
    source: &[u8],
    atlas_width: u32,
    atlas_height: u32,
) -> Result<Vec<FontGlyph>> {
    ensure!(
        source.last() == Some(&0),
        "fullwidth font list has no final terminator"
    );
    ensure!(
        source[..source.len() - 1].len().is_multiple_of(2),
        "fullwidth font list is not a sequence of Shift-JIS pairs"
    );
    source[..source.len() - 1]
        .chunks_exact(2)
        .enumerate()
        .map(|(index, encoded)| {
            let rectangle = GlyphRectangle {
                x: u32::try_from(index)? % FULLWIDTH_COLUMNS * 24,
                y: u32::try_from(index)? / FULLWIDTH_COLUMNS * 24 + 96,
                width: 24,
                height: 24,
            };
            ensure_rectangle(rectangle, atlas_width, atlas_height)?;
            Ok(FontGlyph {
                index,
                source_hex: hex(encoded),
                character: decode_shift_jis(encoded)?,
                rectangle,
            })
        })
        .collect()
}

fn parse_halfwidth_glyphs(
    source: &[u8],
    atlas_width: u32,
    atlas_height: u32,
) -> Result<Vec<FontGlyph>> {
    ensure!(
        source.last() == Some(&0),
        "halfwidth font list has no final terminator"
    );
    source[..source.len() - 1]
        .iter()
        .copied()
        .enumerate()
        .map(|(index, encoded)| {
            ensure!(
                encoded.is_ascii(),
                "halfwidth font list contains a non-ASCII byte"
            );
            let rectangle = GlyphRectangle {
                x: u32::try_from(index)? % HALFWIDTH_COLUMNS * 12 + 12,
                y: u32::try_from(index)? / HALFWIDTH_COLUMNS * 24 + 24,
                width: 12,
                height: 24,
            };
            ensure_rectangle(rectangle, atlas_width, atlas_height)?;
            Ok(FontGlyph {
                index,
                source_hex: format!("{encoded:02x}"),
                character: char::from(encoded).to_string(),
                rectangle,
            })
        })
        .collect()
}

fn ensure_rectangle(rectangle: GlyphRectangle, atlas_width: u32, atlas_height: u32) -> Result<()> {
    ensure!(
        rectangle.x + rectangle.width <= atlas_width
            && rectangle.y + rectangle.height <= atlas_height,
        "font glyph rectangle exceeds the atlas"
    );
    Ok(())
}

fn decode_shift_jis(source: &[u8]) -> Result<String> {
    SHIFT_JIS
        .decode_without_bom_handling_and_without_replacement(source)
        .map(|decoded| decoded.into_owned())
        .context("font list contains invalid Shift-JIS")
}

fn hex(source: &[u8]) -> String {
    source.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "font_atlas/font_atlas_tests.rs"]
mod font_atlas_tests;
