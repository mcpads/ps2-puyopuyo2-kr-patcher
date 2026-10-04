use anyhow::{Context, Result, ensure};
use object::{
    Architecture, BinaryFormat, Endianness, FileFlags, Object, ObjectSection, ObjectSymbol,
};
use serde::Serialize;
use typed_isa_core::StaticSemantics;

const EF_MIPS_MACH: u32 = 0x00ff_0000;
const EF_MIPS_MACH_5900: u32 = 0x0092_0000;

#[derive(Clone, Debug, Serialize)]
pub struct SymbolDisassembly {
    pub schema: u32,
    pub isa_profile: &'static str,
    pub elf_flags: String,
    pub symbol: SymbolIdentity,
    pub instructions: Vec<DecodedInstruction>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableSurvey {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub disassembly: SymbolDisassembly,
}

#[derive(Clone, Debug, Serialize)]
pub struct SymbolIdentity {
    pub name: String,
    pub address: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DecodedInstruction {
    pub address: String,
    pub bytes: String,
    pub canonical: String,
}

#[derive(Clone, Debug)]
pub struct ExecutableSymbolBytes {
    pub address: u64,
    pub file_offset: usize,
    pub bytes: Vec<u8>,
}

pub fn read_executable_symbol(
    executable: &[u8],
    symbol_name: &str,
) -> Result<ExecutableSymbolBytes> {
    ensure!(!symbol_name.is_empty(), "symbol name is empty");
    let file = object::File::parse(executable).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips && file.endianness() == Endianness::Little,
        "executable is not little-endian MIPS ELF"
    );
    let symbol = file
        .symbols()
        .find(|symbol| symbol.name().is_ok_and(|name| name == symbol_name))
        .with_context(|| format!("ELF symbol not found: {symbol_name}"))?;
    ensure!(
        symbol.is_definition() && symbol.size() > 0,
        "ELF symbol has no file-backed definition: {symbol_name}"
    );
    let section_index = symbol
        .section_index()
        .with_context(|| format!("ELF symbol has no section: {symbol_name}"))?;
    let section = file.section_by_index(section_index)?;
    let relative = symbol
        .address()
        .checked_sub(section.address())
        .with_context(|| format!("ELF symbol precedes its section: {symbol_name}"))?;
    let (section_file_offset, section_file_size) = section
        .file_range()
        .with_context(|| format!("ELF symbol section has no file range: {symbol_name}"))?;
    ensure!(
        relative + symbol.size() <= section_file_size,
        "ELF symbol exceeds its file-backed section: {symbol_name}"
    );
    let file_offset = usize::try_from(section_file_offset + relative)?;
    let byte_length = usize::try_from(symbol.size())?;
    let end = file_offset
        .checked_add(byte_length)
        .context("ELF symbol file range overflow")?;
    let bytes = executable
        .get(file_offset..end)
        .with_context(|| format!("ELF symbol exceeds executable: {symbol_name}"))?
        .to_vec();
    Ok(ExecutableSymbolBytes {
        address: symbol.address(),
        file_offset,
        bytes,
    })
}

pub fn disassemble_symbol(executable: &[u8], symbol_name: &str) -> Result<SymbolDisassembly> {
    ensure!(!symbol_name.is_empty(), "symbol name is empty");
    let file = object::File::parse(executable).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips,
        "ELF architecture is not 32-bit MIPS"
    );
    ensure!(
        file.endianness() == Endianness::Little,
        "ELF is not little-endian"
    );
    let elf_flags = match file.flags() {
        FileFlags::Elf { e_flags, .. } => e_flags,
        _ => unreachable!("ELF format already checked"),
    };
    ensure!(
        elf_flags & EF_MIPS_MACH == EF_MIPS_MACH_5900,
        "ELF does not declare the R5900 machine flag: {elf_flags:#010x}"
    );

    let symbol = file
        .symbols()
        .find(|symbol| symbol.name().is_ok_and(|name| name == symbol_name))
        .with_context(|| format!("ELF symbol not found: {symbol_name}"))?;
    ensure!(
        symbol.is_definition(),
        "ELF symbol is not defined: {symbol_name}"
    );
    let address = symbol.address();
    let byte_length = symbol.size();
    ensure!(byte_length > 0, "ELF symbol has no size: {symbol_name}");
    ensure!(
        address.is_multiple_of(4) && byte_length.is_multiple_of(4),
        "ELF symbol is not instruction aligned: {symbol_name}"
    );
    let section_index = symbol
        .section_index()
        .with_context(|| format!("ELF symbol has no section: {symbol_name}"))?;
    let section = file.section_by_index(section_index)?;
    let relative = address
        .checked_sub(section.address())
        .with_context(|| format!("ELF symbol precedes its section: {symbol_name}"))?;
    let start = usize::try_from(relative).context("symbol offset does not fit memory")?;
    let length = usize::try_from(byte_length).context("symbol size does not fit memory")?;
    let end = start
        .checked_add(length)
        .context("symbol range overflows memory")?;
    let section_data = section.data()?;
    let symbol_data = section_data
        .get(start..end)
        .with_context(|| format!("ELF symbol exceeds its section: {symbol_name}"))?;

    let instructions = decode_exact(symbol_data, address)?;
    Ok(SymbolDisassembly {
        schema: 1,
        isa_profile: r5900::PROFILE_ID,
        elf_flags: format!("{elf_flags:#010x}"),
        symbol: SymbolIdentity {
            name: symbol_name.to_string(),
            address: format!("{address:#010x}"),
            byte_length,
        },
        instructions,
    })
}

fn decode_exact(source: &[u8], start_address: u64) -> Result<Vec<DecodedInstruction>> {
    ensure!(
        start_address.is_multiple_of(4) && source.len().is_multiple_of(4),
        "R5900 instruction range is not four-byte aligned"
    );
    source
        .chunks_exact(4)
        .enumerate()
        .map(|(index, bytes)| {
            let address = start_address + (index as u64 * 4);
            let instruction = validate_r5900_instruction(bytes, address)?;
            Ok(DecodedInstruction {
                address: format!("{address:#010x}"),
                bytes: bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
                canonical: r5900::format_instruction(&instruction),
            })
        })
        .collect()
}

pub(crate) fn validate_r5900_instruction(
    source: &[u8],
    address: u64,
) -> Result<r5900::Instruction> {
    ensure!(
        source.len() == 4,
        "R5900 instruction must contain exactly four bytes"
    );
    let location = r5900::CodeLocation::try_from(address)
        .with_context(|| format!("R5900 code location exceeds 32 bits: {address:#x}"))?;
    let instruction = r5900::decode_bytes(source, location)
        .with_context(|| format!("failed to decode R5900 instruction at {address:#x}"))?;
    let encoded = r5900::encode_bytes(&instruction, location)
        .with_context(|| format!("failed to re-encode instruction at {address:#x}"))?;
    ensure!(
        encoded == source,
        "R5900 decode/encode changed bytes at {address:#x}"
    );
    r5900::R5900::semantics(&instruction, &r5900::AnalysisPoint::new(location))
        .with_context(|| format!("failed to derive R5900 semantics at {address:#x}"))?;
    Ok(instruction)
}

#[cfg(test)]
#[path = "executable/executable_tests.rs"]
mod executable_tests;
