use std::collections::BTreeSet;

use anyhow::{Context, Result};
use object::{Object, ObjectSection};
use r5900::{GeneralRegister, InstructionForm, Location, Operands};
use typed_isa_core::AccessKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct KnownValue {
    pub(super) value: u64,
    pub(super) instruction_indices: BTreeSet<usize>,
    pub(super) pointer_slot_addresses: BTreeSet<u32>,
    pub(super) uses_gp_initial_value: bool,
}

impl KnownValue {
    fn constant(value: u64) -> Self {
        Self {
            value,
            instruction_indices: BTreeSet::new(),
            pointer_slot_addresses: BTreeSet::new(),
            uses_gp_initial_value: false,
        }
    }

    fn gp(value: u32) -> Self {
        Self {
            value: u64::from(value),
            instruction_indices: BTreeSet::new(),
            pointer_slot_addresses: BTreeSet::new(),
            uses_gp_initial_value: true,
        }
    }

    fn with_instruction(mut self, index: usize) -> Self {
        self.instruction_indices.insert(index);
        self
    }

    fn combine(mut self, other: &Self) -> Self {
        self.instruction_indices
            .extend(other.instruction_indices.iter().copied());
        self.pointer_slot_addresses
            .extend(other.pointer_slot_addresses.iter().copied());
        self.uses_gp_initial_value |= other.uses_gp_initial_value;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RegisterState {
    values: [Option<KnownValue>; 32],
}

impl RegisterState {
    pub(super) fn unknown() -> Self {
        let mut values = std::array::from_fn(|_| None);
        values[usize::from(GeneralRegister::ZERO.index())] = Some(KnownValue::constant(0));
        Self { values }
    }

    pub(super) fn at_block_entry(gp: u32) -> Self {
        let mut values = Self::unknown().values;
        values[usize::from(GeneralRegister::GP.index())] = Some(KnownValue::gp(gp));
        Self { values }
    }

    fn get(&self, register: GeneralRegister) -> Option<&KnownValue> {
        self.values[usize::from(register.index())].as_ref()
    }

    pub(super) fn clear(&mut self, register: GeneralRegister) {
        if register != GeneralRegister::ZERO {
            self.values[usize::from(register.index())] = None;
        }
    }

    pub(super) fn set(&mut self, register: GeneralRegister, value: KnownValue) {
        if register != GeneralRegister::ZERO {
            self.values[usize::from(register.index())] = Some(value);
        }
    }

    pub(super) fn retain_exact_values_from(&mut self, incoming: &Self) -> bool {
        let mut changed = false;
        for (current, incoming) in self.values.iter_mut().zip(&incoming.values) {
            if current.as_ref() != incoming.as_ref() && current.is_some() {
                *current = None;
                changed = true;
            }
        }
        changed
    }
}

pub(super) fn written_general_registers(
    semantics: &r5900::InstructionSemantics,
) -> BTreeSet<GeneralRegister> {
    semantics
        .location_accesses
        .iter()
        .filter_map(|access| {
            if access.kind != AccessKind::Write {
                return None;
            }
            let Location::General(storage) = access.location else {
                return None;
            };
            Some(storage.register)
        })
        .collect()
}

pub(super) fn compute_known_value(
    state: &RegisterState,
    instruction: &r5900::Instruction,
    index: usize,
    read_u32: &mut impl FnMut(u32) -> Result<Option<u32>>,
) -> Result<Option<(GeneralRegister, KnownValue)>> {
    let form = instruction.form();
    let result = match instruction.operands() {
        Operands::LoadUpper {
            destination,
            immediate,
        } if form == InstructionForm::Lui => Some((
            *destination,
            KnownValue::constant(sign_extend_u32(u32::from(*immediate) << 16))
                .with_instruction(index),
        )),
        Operands::SignedImmediate {
            destination,
            source,
            immediate,
        } if matches!(form, InstructionForm::Addiu | InstructionForm::Daddiu) => {
            state.get(*source).map(|source_value| {
                let value = if form == InstructionForm::Addiu {
                    sign_extend_u32(
                        (source_value.value as u32).wrapping_add_signed(i32::from(*immediate)),
                    )
                } else {
                    source_value
                        .value
                        .wrapping_add_signed(i64::from(*immediate))
                };
                let mut resolved = source_value.clone();
                resolved.value = value;
                (*destination, resolved.with_instruction(index))
            })
        }
        Operands::UnsignedImmediate {
            destination,
            source,
            immediate,
        } if matches!(
            form,
            InstructionForm::Andi | InstructionForm::Ori | InstructionForm::Xori
        ) =>
        {
            state.get(*source).map(|source_value| {
                let immediate = u64::from(*immediate);
                let value = match form {
                    InstructionForm::Andi => source_value.value & immediate,
                    InstructionForm::Ori => source_value.value | immediate,
                    InstructionForm::Xori => source_value.value ^ immediate,
                    _ => unreachable!("form guard limits unsigned operations"),
                };
                let mut resolved = source_value.clone();
                resolved.value = value;
                (*destination, resolved.with_instruction(index))
            })
        }
        Operands::ThreeGeneral {
            destination,
            left,
            right,
        } if matches!(
            form,
            InstructionForm::Addu
                | InstructionForm::Daddu
                | InstructionForm::Subu
                | InstructionForm::Dsubu
                | InstructionForm::And
                | InstructionForm::Or
                | InstructionForm::Xor
                | InstructionForm::Nor
        ) =>
        {
            match (state.get(*left), state.get(*right)) {
                (Some(left_value), Some(right_value)) => {
                    let value = match form {
                        InstructionForm::Addu => sign_extend_u32(
                            (left_value.value as u32).wrapping_add(right_value.value as u32),
                        ),
                        InstructionForm::Daddu => left_value.value.wrapping_add(right_value.value),
                        InstructionForm::Subu => sign_extend_u32(
                            (left_value.value as u32).wrapping_sub(right_value.value as u32),
                        ),
                        InstructionForm::Dsubu => left_value.value.wrapping_sub(right_value.value),
                        InstructionForm::And => left_value.value & right_value.value,
                        InstructionForm::Or => left_value.value | right_value.value,
                        InstructionForm::Xor => left_value.value ^ right_value.value,
                        InstructionForm::Nor => !(left_value.value | right_value.value),
                        _ => unreachable!("form guard limits three-register operations"),
                    };
                    let mut resolved = left_value.clone().combine(right_value);
                    resolved.value = value;
                    Some((*destination, resolved.with_instruction(index)))
                }
                _ => None,
            }
        }
        Operands::ShiftImmediate {
            destination,
            source,
            amount,
        } if matches!(
            form,
            InstructionForm::Sll
                | InstructionForm::Srl
                | InstructionForm::Sra
                | InstructionForm::Dsll
                | InstructionForm::Dsll32
                | InstructionForm::Dsrl
                | InstructionForm::Dsrl32
                | InstructionForm::Dsra
                | InstructionForm::Dsra32
        ) =>
        {
            state.get(*source).map(|source_value| {
                let amount = u32::from(amount.value());
                let value = match form {
                    InstructionForm::Sll => {
                        sign_extend_u32((source_value.value as u32).wrapping_shl(amount))
                    }
                    InstructionForm::Srl => sign_extend_u32((source_value.value as u32) >> amount),
                    InstructionForm::Sra => {
                        sign_extend_u32(((source_value.value as u32) as i32 >> amount) as u32)
                    }
                    InstructionForm::Dsll => source_value.value.wrapping_shl(amount),
                    InstructionForm::Dsll32 => source_value.value.wrapping_shl(amount + 32),
                    InstructionForm::Dsrl => source_value.value >> amount,
                    InstructionForm::Dsrl32 => source_value.value >> (amount + 32),
                    InstructionForm::Dsra => (source_value.value as i64 >> amount) as u64,
                    InstructionForm::Dsra32 => (source_value.value as i64 >> (amount + 32)) as u64,
                    _ => unreachable!("form guard limits shift operations"),
                };
                let mut resolved = source_value.clone();
                resolved.value = value;
                (*destination, resolved.with_instruction(index))
            })
        }
        Operands::MemoryGeneral {
            register,
            base,
            offset,
        } if matches!(form, InstructionForm::Lw | InstructionForm::Lwu) => {
            let Some(base_value) = state.get(*base) else {
                return Ok(None);
            };
            let address = base_value.value.wrapping_add_signed(i64::from(*offset));
            let Ok(address) = u32::try_from(address) else {
                return Ok(None);
            };
            read_u32(address)?.map(|loaded| {
                let mut resolved = base_value.clone();
                resolved.value = if form == InstructionForm::Lw {
                    (i64::from(loaded as i32)) as u64
                } else {
                    u64::from(loaded)
                };
                resolved.pointer_slot_addresses.insert(address);
                (*register, resolved.with_instruction(index))
            })
        }
        _ => None,
    };
    Ok(result)
}

pub(super) fn read_file_backed_u32(file: &object::File<'_>, address: u32) -> Result<Option<u32>> {
    let address = u64::from(address);
    for section in file.sections() {
        let Some(relative) = address.checked_sub(section.address()) else {
            continue;
        };
        let Some(end) = relative.checked_add(4) else {
            continue;
        };
        let Some((_, file_size)) = section.file_range() else {
            continue;
        };
        if end > file_size {
            continue;
        }
        let start = usize::try_from(relative)?;
        let bytes = section
            .data()?
            .get(start..start + 4)
            .context("static pointer word exceeds its section")?;
        return Ok(Some(u32::from_le_bytes(bytes.try_into()?)));
    }
    Ok(None)
}

fn sign_extend_u32(value: u32) -> u64 {
    (i64::from(value as i32)) as u64
}
