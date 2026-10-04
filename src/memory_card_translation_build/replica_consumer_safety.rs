use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection, ObjectSymbol, RelocationFlags, RelocationTarget};
use r5900::{GeneralRegister, InstructionForm, Location, Operands};
use typed_isa_core::{AccessKind, ControlAction, ControlTarget, StaticSemantics};

use super::model::FontReplicaConsumerSafetyReport;
use crate::cpx::read_symbol_bytes;
use crate::executable::read_executable_symbol;
use crate::executable_references::{find_direct_calls, find_static_references};
use crate::indexed_texture::{
    LoadRecord, TextureRegistrationPair, parse_load_records, parse_texture_registration_pairs,
};
use crate::texture_pattern::{TexturePatternRecord, parse_pattern_records};
use crate::typed_evidence::{
    decode_instruction, require_memory, require_shift, require_signed_immediate,
    require_three_general, require_unsigned_immediate, symbol_address, validate_jump_target,
    validate_loaded_address,
};

const G122_IMAGE_FILE_RECORD: usize = 122;
const G122_PALETTE_FILE_RECORD: usize = 242;
const G122_REGISTRATION_SLOT: usize = 1;

#[derive(Clone, Copy)]
struct OwnerLoadTable {
    symbol: &'static str,
    image_loads: usize,
    palette_loads: usize,
    g122_image_load_index: usize,
    g122_palette_slot: usize,
}

const OWNER_LOAD_TABLES: [OwnerLoadTable; 14] = [
    frame_owner("FrameTexTbl_00"),
    frame_owner("FrameTexTbl_01"),
    frame_owner("FrameTexTbl_02"),
    frame_owner("FrameTexTbl_03"),
    frame_owner("FrameTexTbl_04"),
    frame_owner("FrameTexTbl_05"),
    frame_owner("FrameTexTbl_06"),
    frame_owner("FrameTexTbl_07"),
    frame_owner("FrameTexTbl_10"),
    frame_owner("FrameTexTbl_11"),
    frame_owner("FrameTexTbl_12"),
    OwnerLoadTable {
        symbol: "NazoEndTexTbl",
        image_loads: 2,
        palette_loads: 2,
        g122_image_load_index: 1,
        g122_palette_slot: 1,
    },
    OwnerLoadTable {
        symbol: "NazoTexTbl",
        image_loads: 29,
        palette_loads: 16,
        g122_image_load_index: 9,
        g122_palette_slot: 9,
    },
    OwnerLoadTable {
        symbol: "RankTexTbl",
        image_loads: 6,
        palette_loads: 6,
        g122_image_load_index: 5,
        g122_palette_slot: 5,
    },
];

const fn frame_owner(symbol: &'static str) -> OwnerLoadTable {
    OwnerLoadTable {
        symbol,
        image_loads: 2,
        palette_loads: 2,
        g122_image_load_index: 1,
        g122_palette_slot: 1,
    }
}

const FIXED_PATTERN_TABLES: [(&str, usize); 13] = [
    ("GameFramePtn", 12),
    ("GameFramePtnNazo", 6),
    ("GameFrame_Nazo", 2),
    ("GamePtn_Pause", 2),
    ("GamePtn_Result", 2),
    ("RankPtn", 8),
    ("RankPuyoPtn", 3),
    ("NazoStr_Ptn", 58),
    ("NazoMenu_Ptn", 17),
    ("NazoEdit_Ptn", 36),
    ("NazoNazoYN_Ptn", 18),
    ("BlockFrame_Ptn", 19),
    ("NazoEnd_Ptn", 1),
];

const DIRECT_PATTERN_CALL_ADDRESSES: [u64; 27] = [
    0x0042_ffa8,
    0x0042_ffe8,
    0x0043_c8fc,
    0x0043_cb44,
    0x0043_cbc0,
    0x0044_3460,
    0x0044_7754,
    0x0044_777c,
    0x0044_79c0,
    0x0044_79e4,
    0x0044_7d80,
    0x0044_7dc4,
    0x0044_83b4,
    0x0044_9008,
    0x0044_9130,
    0x0045_f010,
    0x0045_f070,
    0x0046_0590,
    0x0046_0724,
    0x0046_0820,
    0x0046_0878,
    0x0046_164c,
    0x0046_1878,
    0x0046_4b2c,
    0x0046_4f6c,
    0x0046_b7e4,
    0x0047_5d5c,
];
const DYNAMIC_SLOT_ONE_PATTERN_CALL_ADDRESSES: [u64; 2] = [0x0042_ffa8, 0x0042_ffe8];
const GAME_PATTERN_RELOCATIONS: [(u64, u32); 4] = [
    (0x0045_ca68, object::elf::R_MIPS_HI16),
    (0x0045_ca7c, object::elf::R_MIPS_LO16),
    (0x0045_ca80, object::elf::R_MIPS_HI16),
    (0x0045_ca94, object::elf::R_MIPS_LO16),
];
const FRAME_SELECTION_TABLES: [(&str, usize); 4] = [
    ("FrameTexTbl_1P_P", 7),
    ("FrameTexTbl_1P_N", 9),
    ("FrameTexTbl_1P_T", 39),
    ("FrameTexTbl_2P", 6),
];
const FRAME_SELECTION_STORAGE_RELOCATIONS: [(u64, u32); 12] = [
    (0x0046_c4a8, object::elf::R_MIPS_GPREL16),
    (0x0046_c87c, object::elf::R_MIPS_GPREL16),
    (0x0046_c880, object::elf::R_MIPS_GPREL16),
    (0x0046_c920, object::elf::R_MIPS_GPREL16),
    (0x0046_cd3c, object::elf::R_MIPS_GPREL16),
    (0x0046_cd40, object::elf::R_MIPS_GPREL16),
    (0x0047_6650, object::elf::R_MIPS_GPREL16),
    (0x0047_6f1c, object::elf::R_MIPS_GPREL16),
    (0x0047_6f48, object::elf::R_MIPS_GPREL16),
    (0x0047_6f78, object::elf::R_MIPS_GPREL16),
    (0x0047_6f9c, object::elf::R_MIPS_GPREL16),
    (0x0047_6fa0, object::elf::R_MIPS_GPREL16),
];

pub(super) fn validate_secondary_font_replica_consumer_scope(
    executable: &[u8],
) -> Result<FontReplicaConsumerSafetyReport> {
    let file = object::File::parse(executable)
        .context("failed to parse executable for G122 replica-consumer validation")?;
    for owner in OWNER_LOAD_TABLES {
        let (_, bytes) = read_symbol_bytes(&file, owner.symbol)?;
        let loads = parse_load_records(&bytes)?;
        validate_owner_load_table(owner, &loads)?;
    }
    validate_frame_and_nazo_end_slot_one_registration(&file)?;
    validate_rank_slot_one_registration(&file)?;

    let mut validated_fixed_pattern_records = 0_usize;
    for (symbol, expected_records) in FIXED_PATTERN_TABLES {
        let (_, bytes) = read_symbol_bytes(&file, symbol)?;
        let records = parse_pattern_records(symbol, &bytes)?;
        ensure!(
            records.len() == expected_records,
            "{symbol} fixed-pattern population changed"
        );
        validate_no_slot_one_patterns(symbol, &records)?;
        validated_fixed_pattern_records = validated_fixed_pattern_records
            .checked_add(records.len())
            .context("G122 fixed-pattern record count overflow")?;
    }
    validate_direct_pattern_call_census(&file, executable)?;
    validate_unreachable_game_pattern_slot_one(&file)?;

    Ok(FontReplicaConsumerSafetyReport {
        owner_load_tables: OWNER_LOAD_TABLES.iter().map(|owner| owner.symbol).collect(),
        owner_load_table_count: OWNER_LOAD_TABLES.len(),
        validated_fixed_pattern_tables: FIXED_PATTERN_TABLES
            .iter()
            .map(|(symbol, _)| *symbol)
            .collect(),
        validated_fixed_pattern_records,
        slot_one_fixed_pattern_records: 0,
        direct_pattern_call_sites: DIRECT_PATTERN_CALL_ADDRESSES.len(),
        dynamic_slot_one_call_sites: DYNAMIC_SLOT_ONE_PATTERN_CALL_ADDRESSES.len(),
        fixed_slot_one_call_sites: 0,
        unreachable_slot_one_pattern_records: 1,
        coverage_status: "closed for the exact-source G122 Frame, Nazo, Nazo-ending, and Rank registration, fixed-pattern, and direct-pattern call census",
        unresolved_scope: "none inside the exact source-bound G122 consumer census; cold-boot behavior remains a separate runtime gate",
        static_consumer_scope_closed: true,
    })
}

mod pattern_usage;
mod registration;

use pattern_usage::{
    validate_direct_pattern_call_census, validate_no_slot_one_patterns,
    validate_rank_slot_one_registration, validate_unreachable_game_pattern_slot_one,
};
use registration::validate_frame_and_nazo_end_slot_one_registration;

fn validate_main_symbol_relocations(
    file: &object::File<'_>,
    target_symbol: &str,
    expected: &[(u64, u32)],
) -> Result<Vec<(u64, u32)>> {
    let actual = main_symbol_relocations(file, target_symbol)?;
    ensure!(
        actual == expected,
        ".relmain {target_symbol} relocation population changed: {actual:?}"
    );
    Ok(actual)
}

fn validate_main_symbol_relocations_of_type(
    file: &object::File<'_>,
    target_symbol: &str,
    relocation_type: u32,
    expected: &[(u64, u32)],
) -> Result<Vec<(u64, u32)>> {
    let actual = main_symbol_relocations(file, target_symbol)?
        .into_iter()
        .filter(|(_, candidate_type)| *candidate_type == relocation_type)
        .collect::<Vec<_>>();
    ensure!(
        actual == expected,
        ".relmain {target_symbol} relocation type {relocation_type} population changed: {actual:?}"
    );
    Ok(actual)
}

fn main_symbol_relocations(
    file: &object::File<'_>,
    target_symbol: &str,
) -> Result<Vec<(u64, u32)>> {
    file.section_by_name(".relmain")
        .context("ELF has no .relmain relocation section")?;
    let main = file
        .section_by_name("main")
        .context("ELF has no main section associated with .relmain")?;
    let target_address = file
        .symbols()
        .find(|symbol| {
            symbol.is_definition()
                && symbol
                    .name()
                    .is_ok_and(|candidate| candidate == target_symbol)
        })
        .with_context(|| format!("ELF has no relocation target symbol {target_symbol}"))?
        .address();
    let mut actual = Vec::new();
    for (address, relocation) in main.relocations() {
        let RelocationTarget::Symbol(symbol_index) = relocation.target() else {
            continue;
        };
        let symbol = file.symbol_by_index(symbol_index)?;
        if symbol.address() != target_address {
            continue;
        }
        let RelocationFlags::Elf { r_type } = relocation.flags() else {
            anyhow::bail!("{target_symbol} relocation is not an ELF relocation")
        };
        actual.push((address, r_type));
    }
    actual.sort_unstable();
    Ok(actual)
}

fn main_relocations_to_addresses(
    file: &object::File<'_>,
    target_addresses: &BTreeSet<u64>,
) -> Result<Vec<(u64, u32, u64)>> {
    file.section_by_name(".relmain")
        .context("ELF has no .relmain relocation section")?;
    let main = file
        .section_by_name("main")
        .context("ELF has no main section associated with .relmain")?;
    let mut actual = Vec::new();
    for (address, relocation) in main.relocations() {
        let RelocationTarget::Symbol(symbol_index) = relocation.target() else {
            continue;
        };
        let target_address = file.symbol_by_index(symbol_index)?.address();
        if !target_addresses.contains(&target_address) {
            continue;
        }
        let RelocationFlags::Elf { r_type } = relocation.flags() else {
            anyhow::bail!("main relocation is not an ELF relocation")
        };
        actual.push((address, r_type, target_address));
    }
    actual.sort_unstable();
    Ok(actual)
}

fn validate_owner_load_table(owner: OwnerLoadTable, loads: &[LoadRecord]) -> Result<()> {
    ensure!(
        loads.len() == owner.image_loads + owner.palette_loads
            && loads[..owner.image_loads]
                .iter()
                .all(|load| load.operation == 1)
            && loads[owner.image_loads..]
                .iter()
                .all(|load| load.operation == 2),
        "{} image/palette load population changed",
        owner.symbol
    );
    let image_loads = &loads[..owner.image_loads];
    let palette_loads = &loads[owner.image_loads..];
    ensure!(
        image_loads.first().is_some_and(|load| load.flags & 1 == 1)
            && image_loads
                .iter()
                .skip(1)
                .all(|load| load.flags & 1 == 0 && load.destination == 0),
        "{} image destination cursor layout changed",
        owner.symbol
    );
    ensure!(
        palette_loads
            .first()
            .is_some_and(|load| load.flags & 1 == 1 && load.destination == 0x1a40)
            && palette_loads
                .iter()
                .skip(1)
                .all(|load| load.flags & 1 == 0 && load.destination == 0),
        "{} palette destination cursor layout changed",
        owner.symbol
    );
    let image = loads
        .get(owner.g122_image_load_index)
        .with_context(|| format!("{} has no G122 image load", owner.symbol))?;
    let palette = loads
        .get(owner.image_loads + owner.g122_palette_slot)
        .with_context(|| format!("{} has no G122 palette load", owner.symbol))?;
    ensure!(
        image.file_record == G122_IMAGE_FILE_RECORD
            && image.flags & 1 == 0
            && image.destination == 0
            && palette.file_record == G122_PALETTE_FILE_RECORD
            && palette.flags & 1 == 0
            && palette.destination == 0,
        "{} no longer binds its main-font clone to G122/G242",
        owner.symbol
    );
    Ok(())
}
