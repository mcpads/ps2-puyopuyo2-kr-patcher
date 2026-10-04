use anyhow::{Context, Result};
use r5900::{GeneralRegister, InstructionForm};
use serde::Serialize;

use crate::cpx::read_symbol_bytes;
use crate::typed_evidence::{
    require_memory, require_shift, require_signed_immediate, require_three_general, symbol_address,
    validate_jump_target,
};

const STORY_TEXTURE_CONSUMER_SYMBOL: &str = "Seq_ActMsgWindow";
const SPRITE_PACKET_CONSUMER_SYMBOL: &str = "aqSetSpritePacket";
const COMMON_SPRITE_ALLOCATOR_SYMBOL: &str = "aqSet_ComSpr";
const MEMORY_COPY_SYMBOL: &str = "memcpy";
const STORY_TEXTURE_CELL_PIXELS: u32 = 24;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoryDialogueTextureCellConsumerValidation {
    pub producer_symbol: String,
    pub packet_consumer_symbol: String,
    pub cell_width_pixels: u32,
    pub cell_height_pixels: u32,
}

pub fn validate_story_dialogue_texture_cell_consumer(
    executable: &[u8],
) -> Result<StoryDialogueTextureCellConsumerValidation> {
    let file = object::File::parse(executable).context("failed to parse story executable")?;
    let (producer_address, producer_bytes) =
        read_symbol_bytes(&file, STORY_TEXTURE_CONSUMER_SYMBOL)?;
    let (packet_consumer_address, packet_consumer_bytes) =
        read_symbol_bytes(&file, SPRITE_PACKET_CONSUMER_SYMBOL)?;
    validate_story_dialogue_texture_cell_instructions(
        &producer_bytes,
        producer_address,
        &packet_consumer_bytes,
        packet_consumer_address,
        symbol_address(&file, COMMON_SPRITE_ALLOCATOR_SYMBOL)?,
        symbol_address(&file, MEMORY_COPY_SYMBOL)?,
    )?;

    Ok(StoryDialogueTextureCellConsumerValidation {
        producer_symbol: STORY_TEXTURE_CONSUMER_SYMBOL.to_string(),
        packet_consumer_symbol: SPRITE_PACKET_CONSUMER_SYMBOL.to_string(),
        cell_width_pixels: STORY_TEXTURE_CELL_PIXELS,
        cell_height_pixels: STORY_TEXTURE_CELL_PIXELS,
    })
}

pub(super) fn validate_story_dialogue_texture_cell_instructions(
    producer: &[u8],
    producer_address: u64,
    packet_consumer: &[u8],
    packet_consumer_address: u64,
    allocator_address: u64,
    memory_copy_address: u64,
) -> Result<()> {
    require_memory(
        producer,
        producer_address,
        0x104,
        InstructionForm::Lw,
        GeneralRegister::S2,
        GeneralRegister::S3,
        36,
        "story common-sprite record pointer",
    )?;

    validate_cell_scale(
        producer,
        producer_address,
        0x388,
        0x38c,
        0x390,
        0x394,
        0x398,
        208,
        12,
        "story texture width",
    )?;
    validate_cell_scale(
        producer,
        producer_address,
        0x39c,
        0x3a0,
        0x3a4,
        0x3a8,
        0x3ac,
        212,
        16,
        "story texture height",
    )?;
    require_memory(
        producer,
        producer_address,
        0x3b0,
        InstructionForm::Sw,
        GeneralRegister::ZERO,
        GeneralRegister::S2,
        20,
        "story texture source x origin",
    )?;
    validate_cell_scale(
        producer,
        producer_address,
        0x3b4,
        0x3b8,
        0x3bc,
        0x3c0,
        0x3c8,
        288,
        24,
        "story texture source row",
    )?;
    validate_jump_target(
        producer,
        producer_address,
        0x3c4,
        allocator_address,
        "story common-sprite allocation",
    )?;
    require_memory(
        producer,
        producer_address,
        0x424,
        InstructionForm::Lw,
        GeneralRegister::A1,
        GeneralRegister::S3,
        36,
        "story common-sprite copy source",
    )?;
    validate_jump_target(
        producer,
        producer_address,
        0x428,
        memory_copy_address,
        "story common-sprite record copy",
    )?;
    require_signed_immediate(
        producer,
        producer_address,
        0x42c,
        InstructionForm::Addiu,
        GeneralRegister::A2,
        GeneralRegister::ZERO,
        72,
        "story common-sprite record byte length",
    )?;

    require_memory(
        packet_consumer,
        packet_consumer_address,
        0x1c8,
        InstructionForm::Lw,
        GeneralRegister::V1,
        GeneralRegister::S4,
        20,
        "sprite packet source x origin",
    )?;
    require_memory(
        packet_consumer,
        packet_consumer_address,
        0x1cc,
        InstructionForm::Lw,
        GeneralRegister::V0,
        GeneralRegister::S4,
        24,
        "sprite packet source y origin",
    )?;
    require_memory(
        packet_consumer,
        packet_consumer_address,
        0x218,
        InstructionForm::Lw,
        GeneralRegister::A2,
        GeneralRegister::S4,
        20,
        "sprite packet source x extent origin",
    )?;
    require_memory(
        packet_consumer,
        packet_consumer_address,
        0x21c,
        InstructionForm::Lw,
        GeneralRegister::A1,
        GeneralRegister::S4,
        12,
        "sprite packet source rectangle width",
    )?;
    require_memory(
        packet_consumer,
        packet_consumer_address,
        0x220,
        InstructionForm::Lw,
        GeneralRegister::V1,
        GeneralRegister::S4,
        24,
        "sprite packet source y extent origin",
    )?;
    require_memory(
        packet_consumer,
        packet_consumer_address,
        0x224,
        InstructionForm::Lw,
        GeneralRegister::V0,
        GeneralRegister::S4,
        16,
        "sprite packet source rectangle height",
    )?;
    require_three_general(
        packet_consumer,
        packet_consumer_address,
        0x22c,
        InstructionForm::Addu,
        GeneralRegister::A1,
        GeneralRegister::A2,
        GeneralRegister::A1,
        "sprite packet source rectangle right edge",
    )?;
    require_three_general(
        packet_consumer,
        packet_consumer_address,
        0x234,
        InstructionForm::Addu,
        GeneralRegister::V0,
        GeneralRegister::V1,
        GeneralRegister::V0,
        "sprite packet source rectangle bottom edge",
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_cell_scale(
    source: &[u8],
    start_address: u64,
    load_offset: usize,
    double_offset: usize,
    triple_offset: usize,
    times_eight_offset: usize,
    store_offset: usize,
    input_displacement: i16,
    output_displacement: i16,
    role: &str,
) -> Result<()> {
    require_memory(
        source,
        start_address,
        load_offset,
        InstructionForm::Lw,
        GeneralRegister::V1,
        GeneralRegister::S3,
        input_displacement,
        role,
    )?;
    require_shift(
        source,
        start_address,
        double_offset,
        InstructionForm::Sll,
        GeneralRegister::V0,
        GeneralRegister::V1,
        1,
        role,
    )?;
    require_three_general(
        source,
        start_address,
        triple_offset,
        InstructionForm::Addu,
        GeneralRegister::V0,
        GeneralRegister::V0,
        GeneralRegister::V1,
        role,
    )?;
    require_shift(
        source,
        start_address,
        times_eight_offset,
        InstructionForm::Sll,
        GeneralRegister::V0,
        GeneralRegister::V0,
        3,
        role,
    )?;
    require_memory(
        source,
        start_address,
        store_offset,
        InstructionForm::Sw,
        GeneralRegister::V0,
        GeneralRegister::S2,
        output_displacement,
        role,
    )?;
    Ok(())
}
