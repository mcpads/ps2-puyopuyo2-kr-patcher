use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection, ObjectSymbol};

use crate::executable::validate_r5900_instruction;
use crate::texture_load_census::{TextureLoadCensus, TextureTableClassification};
use crate::typed_evidence::{TypedInstructionEvidence, instruction_evidence};

pub(super) const FILE_ID_REGISTER: u8 = 4;
pub(super) const LOAD_TABLE_REGISTER: u8 = 5;
const ZERO_REGISTER: u8 = 0;
const MAX_RESOLUTION_DEPTH: usize = 64;

#[derive(Clone)]
pub(super) struct CodeSymbol {
    pub(super) name: String,
    pub(super) address: u64,
    bytes: Vec<u8>,
}

#[derive(Clone)]
pub(super) struct RawCall {
    pub(super) owner: CodeSymbol,
    pub(super) instruction_index: usize,
}

pub(super) struct LoadTableInfo {
    pub(super) file_id: usize,
}

struct PointerTableInfo {
    symbol: String,
    load_table_addresses: Vec<u32>,
}

pub(super) struct AnalysisContext<'a> {
    file: &'a object::File<'a>,
    pub(super) load_tables: BTreeMap<u32, LoadTableInfo>,
    pub(super) load_table_symbols: BTreeMap<u32, String>,
    pointer_tables: BTreeMap<u32, PointerTableInfo>,
    pub(super) symbols_by_address: BTreeMap<u32, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AbstractValue {
    Constant(u32),
    IndexedAddress(u32),
    LoadTableSet {
        pointer_table_symbol: String,
        addresses: BTreeSet<u32>,
    },
    Unknown,
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedValue {
    pub(super) value: AbstractValue,
    pub(super) instruction_indices: BTreeSet<usize>,
}

impl ResolvedValue {
    fn constant(value: u32) -> Self {
        Self {
            value: AbstractValue::Constant(value),
            instruction_indices: BTreeSet::new(),
        }
    }

    fn unknown() -> Self {
        Self {
            value: AbstractValue::Unknown,
            instruction_indices: BTreeSet::new(),
        }
    }

    fn with_instruction(mut self, index: usize) -> Self {
        self.instruction_indices.insert(index);
        self
    }
}

impl<'a> AnalysisContext<'a> {
    pub(super) fn from_census(
        file: &'a object::File<'a>,
        census: &TextureLoadCensus,
    ) -> Result<Self> {
        let candidate_addresses = census
            .candidates
            .iter()
            .map(|candidate| {
                Ok((
                    candidate.symbol.as_str(),
                    parse_address(&candidate.address)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut load_tables = BTreeMap::new();
        let mut load_table_symbols = BTreeMap::new();
        for candidate in &census.candidates {
            let TextureTableClassification::LoadTable {
                compatible_sources, ..
            } = &candidate.classification
            else {
                continue;
            };
            let [source] = compatible_sources.as_slice() else {
                anyhow::bail!("load table does not have exactly one file source");
            };
            let address = candidate_addresses[candidate.symbol.as_str()];
            load_tables.insert(
                address,
                LoadTableInfo {
                    file_id: source.file_id,
                },
            );
            load_table_symbols.insert(address, candidate.symbol.clone());
        }
        let mut pointer_tables = BTreeMap::new();
        for candidate in &census.candidates {
            let TextureTableClassification::PointerTable {
                load_table_symbols, ..
            } = &candidate.classification
            else {
                continue;
            };
            pointer_tables.insert(
                candidate_addresses[candidate.symbol.as_str()],
                PointerTableInfo {
                    symbol: candidate.symbol.clone(),
                    load_table_addresses: load_table_symbols
                        .iter()
                        .map(|symbol| {
                            candidate_addresses
                                .get(symbol.as_str())
                                .copied()
                                .with_context(|| {
                                    format!("pointer table names an unknown load table: {symbol}")
                                })
                        })
                        .collect::<Result<_>>()?,
                },
            );
        }
        let symbols_by_address = file
            .symbols()
            .filter_map(|symbol| {
                let address = u32::try_from(symbol.address()).ok()?;
                let name = symbol.name().ok()?.to_string();
                Some((address, name))
            })
            .collect();
        Ok(Self {
            file,
            load_tables,
            load_table_symbols,
            pointer_tables,
            symbols_by_address,
        })
    }
}

pub(super) fn find_calls(file: &object::File<'_>, target: u64) -> Result<Vec<RawCall>> {
    let mut calls = BTreeMap::<u64, RawCall>::new();
    for symbol in file.symbols() {
        let Ok(name) = symbol.name() else {
            continue;
        };
        let Some(bytes) = read_file_backed_symbol(file, &symbol)? else {
            continue;
        };
        if !symbol.address().is_multiple_of(4) || !bytes.len().is_multiple_of(4) {
            continue;
        }
        let owner = CodeSymbol {
            name: name.to_string(),
            address: symbol.address(),
            bytes,
        };
        for (instruction_index, bytes) in owner.bytes.chunks_exact(4).enumerate() {
            let word = u32::from_le_bytes(bytes.try_into()?);
            if word >> 26 != 0x03 {
                continue;
            }
            let address = owner.address + u64::try_from(instruction_index * 4)?;
            let instruction = validate_r5900_instruction(bytes, address)?;
            if instruction.form() != r5900::InstructionForm::Jal
                || !matches!(instruction.operands(), r5900::Operands::Jump { target: actual } if u64::from(*actual) == target)
            {
                continue;
            }
            let call = RawCall {
                owner: owner.clone(),
                instruction_index,
            };
            if let Some(previous) = calls.get(&address) {
                ensure!(
                    previous.owner.name == call.owner.name,
                    "read-file call belongs to overlapping ELF symbols"
                );
            } else {
                calls.insert(address, call);
            }
        }
    }
    Ok(calls.into_values().collect())
}

pub(super) fn resolve_register(
    context: &AnalysisContext<'_>,
    owner: &CodeSymbol,
    register: u8,
    before_index: usize,
    depth: usize,
) -> Result<ResolvedValue> {
    ensure!(
        depth < MAX_RESOLUTION_DEPTH,
        "register resolution exceeded its depth limit"
    );
    if register == ZERO_REGISTER {
        return Ok(ResolvedValue::constant(0));
    }
    let last_index = before_index.min(owner.bytes.len() / 4 - 1);
    for index in (0..=last_index).rev() {
        let word = word_at(owner, index)?;
        if destination_register(word) != Some(register) {
            continue;
        }
        validate_instruction_at(owner, index)?;
        return resolve_definition(context, owner, word, index, depth + 1);
    }
    Ok(ResolvedValue::unknown())
}

fn resolve_definition(
    context: &AnalysisContext<'_>,
    owner: &CodeSymbol,
    word: u32,
    index: usize,
    depth: usize,
) -> Result<ResolvedValue> {
    let opcode = word >> 26;
    let source = register_rs(word);
    let target = register_rt(word);
    match opcode {
        0x0f => Ok(ResolvedValue::constant(u32::from(word as u16) << 16).with_instruction(index)),
        0x09 | 0x19 => {
            let mut resolved = resolve_register_before(context, owner, source, index, depth)?;
            if let AbstractValue::Constant(value) = resolved.value {
                resolved.value = AbstractValue::Constant(
                    value.wrapping_add_signed(i32::from(word as u16 as i16)),
                );
            } else {
                resolved.value = AbstractValue::Unknown;
            }
            Ok(resolved.with_instruction(index))
        }
        0x0d => {
            let mut resolved = resolve_register_before(context, owner, source, index, depth)?;
            if let AbstractValue::Constant(value) = resolved.value {
                resolved.value = AbstractValue::Constant(value | u32::from(word as u16));
            } else {
                resolved.value = AbstractValue::Unknown;
            }
            Ok(resolved.with_instruction(index))
        }
        0x23 => resolve_memory_load(
            context,
            owner,
            target,
            source,
            word as u16 as i16,
            index,
            depth,
        ),
        0x00 => resolve_register_operation(context, owner, word, index, depth),
        _ => Ok(ResolvedValue::unknown().with_instruction(index)),
    }
}

fn resolve_register_operation(
    context: &AnalysisContext<'_>,
    owner: &CodeSymbol,
    word: u32,
    index: usize,
    depth: usize,
) -> Result<ResolvedValue> {
    let function = word & 0x3f;
    match function {
        0x21 | 0x2d | 0x25 => {
            let left = resolve_register_before(context, owner, register_rs(word), index, depth)?;
            let right = resolve_register_before(context, owner, register_rt(word), index, depth)?;
            Ok(combine_add_values(left, right).with_instruction(index))
        }
        0x00 => {
            let mut resolved =
                resolve_register_before(context, owner, register_rt(word), index, depth)?;
            if let AbstractValue::Constant(value) = resolved.value {
                resolved.value = AbstractValue::Constant(value << ((word >> 6) & 0x1f));
            } else {
                resolved.value = AbstractValue::Unknown;
            }
            Ok(resolved.with_instruction(index))
        }
        _ => Ok(ResolvedValue::unknown().with_instruction(index)),
    }
}

fn combine_add_values(mut left: ResolvedValue, right: ResolvedValue) -> ResolvedValue {
    left.instruction_indices
        .extend(right.instruction_indices.iter().copied());
    left.value = match (left.value, right.value) {
        (AbstractValue::Constant(left), AbstractValue::Constant(right)) => {
            AbstractValue::Constant(left.wrapping_add(right))
        }
        (AbstractValue::Constant(base), AbstractValue::Unknown)
        | (AbstractValue::Unknown, AbstractValue::Constant(base)) => {
            AbstractValue::IndexedAddress(base)
        }
        (AbstractValue::IndexedAddress(base), AbstractValue::Constant(offset))
        | (AbstractValue::Constant(offset), AbstractValue::IndexedAddress(base)) => {
            AbstractValue::IndexedAddress(base.wrapping_add(offset))
        }
        _ => AbstractValue::Unknown,
    };
    left
}

fn resolve_memory_load(
    context: &AnalysisContext<'_>,
    owner: &CodeSymbol,
    _destination: u8,
    base: u8,
    offset: i16,
    index: usize,
    depth: usize,
) -> Result<ResolvedValue> {
    if let Some((store_index, stored_register)) = find_previous_store(owner, base, offset, index)? {
        let mut resolved =
            resolve_register_before(context, owner, stored_register, store_index, depth)?;
        resolved.instruction_indices.insert(store_index);
        resolved.instruction_indices.insert(index);
        return Ok(resolved);
    }
    let mut resolved = resolve_register_before(context, owner, base, index, depth)?;
    resolved.value = match resolved.value {
        AbstractValue::Constant(base_address) => {
            let address = base_address.wrapping_add_signed(i32::from(offset));
            resolve_static_pointer(context, address)?.unwrap_or(AbstractValue::Unknown)
        }
        AbstractValue::IndexedAddress(base_address) => {
            let address = base_address.wrapping_add_signed(i32::from(offset));
            context
                .pointer_tables
                .get(&address)
                .map(pointer_table_value)
                .unwrap_or(AbstractValue::Unknown)
        }
        _ => AbstractValue::Unknown,
    };
    Ok(resolved.with_instruction(index))
}

fn resolve_static_pointer(
    context: &AnalysisContext<'_>,
    address: u32,
) -> Result<Option<AbstractValue>> {
    if let Some(pointer_table) = context.pointer_tables.get(&address) {
        return Ok(Some(pointer_table_value(pointer_table)));
    }
    for (base, pointer_table) in &context.pointer_tables {
        let Some(relative) = address.checked_sub(*base) else {
            continue;
        };
        if relative.is_multiple_of(4)
            && usize::try_from(relative / 4)? < pointer_table.load_table_addresses.len()
        {
            let load_table_address =
                pointer_table.load_table_addresses[usize::try_from(relative / 4)?];
            return Ok(Some(AbstractValue::Constant(load_table_address)));
        }
    }
    let Some(value) = read_u32_virtual(context.file, u64::from(address))? else {
        return Ok(None);
    };
    if context.load_tables.contains_key(&value) {
        return Ok(Some(AbstractValue::Constant(value)));
    }
    Ok(None)
}

fn pointer_table_value(pointer_table: &PointerTableInfo) -> AbstractValue {
    AbstractValue::LoadTableSet {
        pointer_table_symbol: pointer_table.symbol.clone(),
        addresses: pointer_table.load_table_addresses.iter().copied().collect(),
    }
}

fn find_previous_store(
    owner: &CodeSymbol,
    base: u8,
    offset: i16,
    before_index: usize,
) -> Result<Option<(usize, u8)>> {
    for index in (0..before_index).rev() {
        let word = word_at(owner, index)?;
        if word >> 26 == 0x2b && register_rs(word) == base && word as u16 as i16 == offset {
            validate_instruction_at(owner, index)?;
            return Ok(Some((index, register_rt(word))));
        }
    }
    Ok(None)
}

fn resolve_register_before(
    context: &AnalysisContext<'_>,
    owner: &CodeSymbol,
    register: u8,
    index: usize,
    depth: usize,
) -> Result<ResolvedValue> {
    if index == 0 {
        return Ok(if register == ZERO_REGISTER {
            ResolvedValue::constant(0)
        } else {
            ResolvedValue::unknown()
        });
    }
    resolve_register(context, owner, register, index - 1, depth)
}

fn destination_register(word: u32) -> Option<u8> {
    match word >> 26 {
        0x00 => match word & 0x3f {
            0x00..=0x07 | 0x09 | 0x0a..=0x0b | 0x10 | 0x12 | 0x20..=0x2f => Some(register_rd(word)),
            _ => None,
        },
        0x08..=0x0f | 0x18..=0x1b | 0x20..=0x27 | 0x30 | 0x34 | 0x37 => Some(register_rt(word)),
        _ => None,
    }
}

fn register_rs(word: u32) -> u8 {
    ((word >> 21) & 0x1f) as u8
}

fn register_rt(word: u32) -> u8 {
    ((word >> 16) & 0x1f) as u8
}

fn register_rd(word: u32) -> u8 {
    ((word >> 11) & 0x1f) as u8
}

fn word_at(owner: &CodeSymbol, index: usize) -> Result<u32> {
    let offset = index
        .checked_mul(4)
        .context("instruction offset overflow")?;
    let bytes = owner
        .bytes
        .get(offset..offset + 4)
        .context("instruction index is out of range")?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

pub(super) fn validate_instruction_at(
    owner: &CodeSymbol,
    index: usize,
) -> Result<r5900::Instruction> {
    let offset = index
        .checked_mul(4)
        .context("instruction offset overflow")?;
    let bytes = owner
        .bytes
        .get(offset..offset + 4)
        .context("instruction index is out of range")?;
    validate_r5900_instruction(bytes, owner.address + u64::try_from(offset)?)
        .with_context(|| format!("failed to validate {} instruction {index}", owner.name))
}

pub(super) fn typed_evidence_at(
    owner: &CodeSymbol,
    index: usize,
) -> Result<TypedInstructionEvidence> {
    let offset = index
        .checked_mul(4)
        .context("instruction offset overflow")?;
    let bytes = owner
        .bytes
        .get(offset..offset + 4)
        .context("instruction index is out of range")?;
    let instruction = validate_r5900_instruction(bytes, owner.address + u64::try_from(offset)?)?;
    Ok(instruction_evidence(
        bytes,
        owner.address + u64::try_from(offset)?,
        &instruction,
    ))
}

fn read_file_backed_symbol(
    file: &object::File<'_>,
    symbol: &object::Symbol<'_, '_>,
) -> Result<Option<Vec<u8>>> {
    if !symbol.is_definition() || symbol.size() == 0 {
        return Ok(None);
    }
    let Some(section_index) = symbol.section_index() else {
        return Ok(None);
    };
    let section = file.section_by_index(section_index)?;
    let Some(relative) = symbol.address().checked_sub(section.address()) else {
        return Ok(None);
    };
    let Some((_, file_size)) = section.file_range() else {
        return Ok(None);
    };
    if relative + symbol.size() > file_size {
        return Ok(None);
    }
    let start = usize::try_from(relative)?;
    let end = start
        .checked_add(usize::try_from(symbol.size())?)
        .context("symbol range overflow")?;
    Ok(section.data()?.get(start..end).map(<[u8]>::to_vec))
}

fn read_u32_virtual(file: &object::File<'_>, address: u64) -> Result<Option<u32>> {
    for section in file.sections() {
        if address < section.address()
            || address + 4 > section.address().saturating_add(section.size())
        {
            continue;
        }
        let Some((_, file_size)) = section.file_range() else {
            continue;
        };
        let relative = address - section.address();
        if relative + 4 > file_size {
            return Ok(None);
        }
        let offset = usize::try_from(relative)?;
        let bytes = section
            .data()?
            .get(offset..offset + 4)
            .context("virtual word exceeds its section")?;
        return Ok(Some(u32::from_le_bytes(bytes.try_into()?)));
    }
    Ok(None)
}

fn parse_address(address: &str) -> Result<u32> {
    u32::from_str_radix(
        address
            .strip_prefix("0x")
            .context("ELF address lacks 0x prefix")?,
        16,
    )
    .context("ELF address is invalid")
}

#[cfg(test)]
#[path = "texture_load_calls_tests.rs"]
mod texture_load_calls_tests;
