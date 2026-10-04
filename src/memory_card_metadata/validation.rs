use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSymbol};
use r5900::{GeneralRegister, InstructionForm, Operands};

use super::MemoryCardMetadataFunction;
use crate::executable::{disassemble_symbol, read_executable_symbol};
use crate::typed_evidence::{
    TypedInstructionEvidence, decode_instruction, instruction_evidence, require_memory,
    require_signed_immediate, require_three_general, symbol_address, validate_jump_target,
    validate_loaded_address,
};

const TITLE_SYMBOL: &str = "@2081";
const HEADER_SYMBOL: &str = "@2082";
const DIRECTORY_SYMBOL: &str = "@1756";
const PATH_FORMAT_SYMBOL: &str = "@1757";
const FILENAME_SYMBOL: &str = "@1759";
const ICON_STRUCTURE_SYMBOL: &str = "mc_IconSys";
const WORK_PATH_SYMBOL: &str = "mc_WorkName";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SymbolStorageIdentity {
    pub address: u64,
    pub byte_length: usize,
}

pub(super) struct ValidatedMetadataPipeline {
    pub icon_structure: SymbolStorageIdentity,
    pub producer: MemoryCardMetadataFunction,
    pub creation_queue: MemoryCardMetadataFunction,
    pub dispatcher: MemoryCardMetadataFunction,
    pub path_builder: MemoryCardMetadataFunction,
    pub dispatch_table_address: u64,
    pub dispatch_entries: usize,
    pub open_handler_address: u64,
    pub write_handler_address: u64,
    pub directory: String,
    pub filename: String,
}

pub(super) fn validate_metadata_pipeline(
    executable: &[u8],
    file: &object::File<'_>,
) -> Result<ValidatedMetadataPipeline> {
    let title_address = symbol_address(file, TITLE_SYMBOL)?;
    let header_address = symbol_address(file, HEADER_SYMBOL)?;
    let icon_structure = symbol_identity(file, ICON_STRUCTURE_SYMBOL)?;
    let strcpy_address = symbol_address(file, "strcpy")?;
    let producer = validate_producer(executable, title_address, header_address, strcpy_address)?;
    let creation_queue = validate_creation_queue(executable, file, icon_structure.address)?;
    let dispatch_table = read_executable_symbol(executable, "@1762")?;
    let dispatch_targets = read_pointer_table(&dispatch_table.bytes, 17)?;
    let open_handler_address = u64::from(dispatch_targets[0x0a]);
    let write_handler_address = u64::from(dispatch_targets[0x0f]);
    ensure!(
        open_handler_address == 0x0042_8788 && write_handler_address == 0x0042_8848,
        "memory-card dispatch handlers changed"
    );
    let dispatcher = validate_dispatcher(
        executable,
        file,
        open_handler_address,
        write_handler_address,
    )?;
    let directory = read_exact_ascii(executable, DIRECTORY_SYMBOL, "/BISLPM-62400")?;
    let filename = read_exact_ascii(executable, FILENAME_SYMBOL, "icon.sys")?;
    let path_format = read_exact_ascii(executable, PATH_FORMAT_SYMBOL, "%s/%s")?;
    ensure!(path_format == "%s/%s", "memory-card path format changed");
    let path_builder = validate_path_builder(executable, file)?;
    Ok(ValidatedMetadataPipeline {
        icon_structure,
        producer,
        creation_queue,
        dispatcher,
        path_builder,
        dispatch_table_address: dispatch_table.address,
        dispatch_entries: dispatch_targets.len(),
        open_handler_address,
        write_handler_address,
        directory,
        filename,
    })
}

fn validate_producer(
    executable: &[u8],
    title_address: u64,
    header_address: u64,
    strcpy_address: u64,
) -> Result<MemoryCardMetadataFunction> {
    let symbol = read_executable_symbol(executable, "Memcard_SetIconSys")?;
    let mut invariants = Vec::new();
    invariants.push(require_three_general(
        &symbol.bytes,
        symbol.address,
        0x18,
        InstructionForm::Daddu,
        GeneralRegister::S2,
        GeneralRegister::A0,
        GeneralRegister::ZERO,
        "icon structure argument preservation",
    )?);
    invariants.extend(validate_loaded_address(
        &symbol.bytes,
        symbol.address,
        0x24,
        0x30,
        GeneralRegister::A1,
        title_address,
        "memory-card title",
    )?);
    invariants.push(require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x28,
        InstructionForm::Addiu,
        GeneralRegister::A0,
        GeneralRegister::S2,
        0xc0,
        "memory-card title field",
    )?);
    invariants.push(validate_jump_target(
        &symbol.bytes,
        symbol.address,
        0x2c,
        strcpy_address,
        "memory-card title copy",
    )?);
    invariants.push(require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x34,
        InstructionForm::Addiu,
        GeneralRegister::V1,
        GeneralRegister::ZERO,
        10,
        "memory-card title newline offset",
    )?);
    invariants.extend(validate_loaded_address(
        &symbol.bytes,
        symbol.address,
        0x38,
        0x50,
        GeneralRegister::A1,
        header_address,
        "memory-card PS2D header",
    )?);
    invariants.push(require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x3c,
        InstructionForm::Addiu,
        GeneralRegister::V0,
        GeneralRegister::ZERO,
        0x60,
        "memory-card icon transparency",
    )?);
    invariants.push(require_memory(
        &symbol.bytes,
        symbol.address,
        0x40,
        InstructionForm::Sh,
        GeneralRegister::V1,
        GeneralRegister::S2,
        6,
        "memory-card title newline field",
    )?);
    invariants.push(require_three_general(
        &symbol.bytes,
        symbol.address,
        0x44,
        InstructionForm::Daddu,
        GeneralRegister::A0,
        GeneralRegister::S2,
        GeneralRegister::ZERO,
        "memory-card header destination",
    )?);
    invariants.push(require_memory(
        &symbol.bytes,
        symbol.address,
        0x48,
        InstructionForm::Sw,
        GeneralRegister::V0,
        GeneralRegister::S2,
        0x0c,
        "memory-card transparency field",
    )?);
    invariants.push(validate_jump_target(
        &symbol.bytes,
        symbol.address,
        0x4c,
        strcpy_address,
        "memory-card header copy",
    )?);
    finish_function(executable, "Memcard_SetIconSys", symbol, invariants)
}

fn validate_creation_queue(
    executable: &[u8],
    file: &object::File<'_>,
    icon_structure_address: u64,
) -> Result<MemoryCardMetadataFunction> {
    let symbol = read_executable_symbol(executable, "Memcard_CreateSys")?;
    let set_icon_address = symbol_address(file, "Memcard_SetIconSys")?;
    let mut invariants = validate_loaded_address(
        &symbol.bytes,
        symbol.address,
        0x50,
        0x58,
        GeneralRegister::A0,
        icon_structure_address,
        "mc_IconSys setup buffer",
    )?;
    invariants.push(validate_jump_target(
        &symbol.bytes,
        symbol.address,
        0x54,
        set_icon_address,
        "Memcard_SetIconSys",
    )?);
    invariants.push(require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x44c,
        InstructionForm::Addiu,
        GeneralRegister::A0,
        GeneralRegister::ZERO,
        0x0f,
        "queued icon.sys write opcode",
    )?);
    invariants.push(require_memory(
        &symbol.bytes,
        symbol.address,
        0x458,
        InstructionForm::Sw,
        GeneralRegister::A0,
        GeneralRegister::V0,
        0,
        "queued icon.sys write opcode store",
    )?);
    invariants.extend(validate_loaded_address(
        &symbol.bytes,
        symbol.address,
        0x45c,
        0x464,
        GeneralRegister::V1,
        icon_structure_address,
        "queued icon.sys write buffer",
    )?);
    invariants.push(require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x46c,
        InstructionForm::Addiu,
        GeneralRegister::A0,
        GeneralRegister::ZERO,
        0x3c4,
        "queued icon.sys write length",
    )?);
    invariants.push(require_memory(
        &symbol.bytes,
        symbol.address,
        0x474,
        InstructionForm::Sw,
        GeneralRegister::V1,
        GeneralRegister::V0,
        0,
        "queued icon.sys write buffer store",
    )?);
    invariants.push(require_memory(
        &symbol.bytes,
        symbol.address,
        0x488,
        InstructionForm::Sw,
        GeneralRegister::A0,
        GeneralRegister::V1,
        0,
        "queued icon.sys write length store",
    )?);
    invariants.push(require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x4d4,
        InstructionForm::Addiu,
        GeneralRegister::A1,
        GeneralRegister::ZERO,
        0x0a,
        "queued icon.sys open opcode",
    )?);
    invariants.push(require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x4dc,
        InstructionForm::Addiu,
        GeneralRegister::A0,
        GeneralRegister::ZERO,
        1,
        "queued icon.sys path selector",
    )?);
    invariants.push(require_memory(
        &symbol.bytes,
        symbol.address,
        0x4e4,
        InstructionForm::Sw,
        GeneralRegister::A1,
        GeneralRegister::V0,
        0,
        "queued icon.sys open opcode store",
    )?);
    invariants.push(require_memory(
        &symbol.bytes,
        symbol.address,
        0x4f8,
        InstructionForm::Sw,
        GeneralRegister::A0,
        GeneralRegister::V1,
        0,
        "queued icon.sys path selector store",
    )?);
    finish_function(executable, "Memcard_CreateSys", symbol, invariants)
}

fn validate_dispatcher(
    executable: &[u8],
    file: &object::File<'_>,
    open_handler_address: u64,
    write_handler_address: u64,
) -> Result<MemoryCardMetadataFunction> {
    let symbol = read_executable_symbol(executable, "Memcard_Sync")?;
    ensure!(
        symbol.address + 0x398 == open_handler_address
            && symbol.address + 0x458 == write_handler_address,
        "memory-card dispatch table no longer selects expected code blocks"
    );
    let open_address = symbol_address(file, "BkupTask_Open")?;
    let write_address = symbol_address(file, "sceMcWrite")?;
    let mut invariants = vec![validate_jump_target(
        &symbol.bytes,
        symbol.address,
        0x398,
        open_address,
        "icon.sys open dispatcher",
    )?];
    invariants.push(require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x39c,
        InstructionForm::Addiu,
        GeneralRegister::A0,
        GeneralRegister::ZERO,
        1,
        "icon.sys write-create open mode",
    )?);
    invariants.push(require_memory(
        &symbol.bytes,
        symbol.address,
        0x474,
        InstructionForm::Lw,
        GeneralRegister::A1,
        GeneralRegister::V0,
        0,
        "sceMcWrite buffer argument",
    )?);
    invariants.push(validate_jump_target(
        &symbol.bytes,
        symbol.address,
        0x478,
        write_address,
        "icon.sys sceMcWrite dispatcher",
    )?);
    invariants.push(require_memory(
        &symbol.bytes,
        symbol.address,
        0x47c,
        InstructionForm::Lw,
        GeneralRegister::A2,
        GeneralRegister::V0,
        4,
        "sceMcWrite length argument",
    )?);
    finish_function(executable, "Memcard_Sync", symbol, invariants)
}

fn validate_path_builder(
    executable: &[u8],
    file: &object::File<'_>,
) -> Result<MemoryCardMetadataFunction> {
    let symbol = read_executable_symbol(executable, "BkupTask_Open")?;
    let mut invariants = vec![require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x4c,
        InstructionForm::Addiu,
        GeneralRegister::V0,
        GeneralRegister::ZERO,
        1,
        "icon.sys path selector comparison",
    )?];
    invariants.push(require_branch_two(
        &symbol.bytes,
        symbol.address,
        0x50,
        GeneralRegister::V1,
        GeneralRegister::V0,
        symbol.address + 0x88,
        "icon.sys path selector branch",
    )?);
    invariants.extend(validate_loaded_address(
        &symbol.bytes,
        symbol.address,
        0x88,
        0x98,
        GeneralRegister::A0,
        symbol_address(file, WORK_PATH_SYMBOL)?,
        "memory-card work path",
    )?);
    invariants.extend(validate_loaded_address(
        &symbol.bytes,
        symbol.address,
        0x8c,
        0x9c,
        GeneralRegister::A1,
        symbol_address(file, PATH_FORMAT_SYMBOL)?,
        "icon.sys path format",
    )?);
    invariants.extend(validate_loaded_address(
        &symbol.bytes,
        symbol.address,
        0x90,
        0xa0,
        GeneralRegister::A2,
        symbol_address(file, DIRECTORY_SYMBOL)?,
        "memory-card save directory",
    )?);
    invariants.extend(validate_loaded_address(
        &symbol.bytes,
        symbol.address,
        0x94,
        0xa8,
        GeneralRegister::A3,
        symbol_address(file, FILENAME_SYMBOL)?,
        "icon.sys filename",
    )?);
    invariants.push(validate_jump_target(
        &symbol.bytes,
        symbol.address,
        0xa4,
        symbol_address(file, "sprintf")?,
        "icon.sys path formatting",
    )?);
    invariants.push(require_branch_two(
        &symbol.bytes,
        symbol.address,
        0x140,
        GeneralRegister::S0,
        GeneralRegister::ZERO,
        symbol.address + 0x168,
        "memory-card open mode branch",
    )?);
    invariants.push(validate_jump_target(
        &symbol.bytes,
        symbol.address,
        0x158,
        symbol_address(file, "sceMcOpen")?,
        "icon.sys sceMcOpen",
    )?);
    invariants.push(require_signed_immediate(
        &symbol.bytes,
        symbol.address,
        0x15c,
        InstructionForm::Addiu,
        GeneralRegister::A3,
        GeneralRegister::ZERO,
        0x0203,
        "icon.sys write-create flags",
    )?);
    finish_function(executable, "BkupTask_Open", symbol, invariants)
}

fn require_branch_two(
    source: &[u8],
    start_address: u64,
    offset: usize,
    left: GeneralRegister,
    right: GeneralRegister,
    target: u64,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == InstructionForm::Beq
            && matches!(
                instruction.operands(),
                Operands::BranchTwo {
                    left: actual_left,
                    right: actual_right,
                    target: actual_target,
                } if *actual_left == left
                    && *actual_right == right
                    && u64::from(*actual_target) == target
            ),
        "{role} changed"
    );
    Ok(instruction_evidence(
        &source[offset..offset + 4],
        start_address + u64::try_from(offset)?,
        &instruction,
    ))
}

fn finish_function(
    executable: &[u8],
    name: &'static str,
    symbol: crate::executable::ExecutableSymbolBytes,
    mut invariants: Vec<TypedInstructionEvidence>,
) -> Result<MemoryCardMetadataFunction> {
    let disassembly = disassemble_symbol(executable, name)?;
    ensure!(
        disassembly.instructions.len() * 4 == symbol.bytes.len(),
        "{name} typed instruction population differs from symbol length"
    );
    invariants.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(MemoryCardMetadataFunction {
        symbol: name,
        address: format!("{:#010x}", symbol.address),
        byte_length: symbol.bytes.len(),
        sha256: crate::indexed_texture::sha256(&symbol.bytes),
        instruction_count: disassembly.instructions.len(),
        invariants,
    })
}

fn symbol_identity(file: &object::File<'_>, name: &str) -> Result<SymbolStorageIdentity> {
    let address = symbol_address(file, name)?;
    let symbol = file
        .symbols()
        .find(|symbol| {
            symbol.address() == address && symbol.name().is_ok_and(|value| value == name)
        })
        .with_context(|| format!("ELF symbol not found: {name}"))?;
    Ok(SymbolStorageIdentity {
        address,
        byte_length: usize::try_from(symbol.size())?,
    })
}

fn read_pointer_table(source: &[u8], expected_entries: usize) -> Result<Vec<u32>> {
    ensure!(
        source.len() == expected_entries * 4,
        "memory-card dispatch table length changed"
    );
    source
        .chunks_exact(4)
        .map(|bytes| Ok(u32::from_le_bytes(bytes.try_into()?)))
        .collect()
}

fn read_exact_ascii(executable: &[u8], symbol: &str, expected: &str) -> Result<String> {
    let storage = read_executable_symbol(executable, symbol)?;
    ensure!(
        !storage.bytes.is_empty()
            && storage.bytes.last() == Some(&0)
            && !storage.bytes[..storage.bytes.len() - 1].contains(&0),
        "{symbol} is not one exact NUL-terminated string"
    );
    let value = std::str::from_utf8(&storage.bytes[..storage.bytes.len() - 1])?;
    ensure!(value == expected, "{symbol} changed");
    Ok(value.to_string())
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod validation_tests;
