use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSymbol};
use r5900::{GeneralRegister, InstructionForm, Operands};
use serde::Serialize;

use crate::executable::validate_r5900_instruction;

#[derive(Clone, Debug, Serialize)]
pub struct TypedInstructionEvidence {
    pub address: String,
    pub bytes: String,
    pub canonical: String,
}

pub(crate) fn decode_instruction(
    source: &[u8],
    start_address: u64,
    offset: usize,
) -> Result<r5900::Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .context("typed instruction offset is out of range")?;
    let address = start_address + u64::try_from(offset)?;
    validate_r5900_instruction(bytes, address)
        .with_context(|| format!("failed to validate typed instruction at {address:#x}"))
}

pub(crate) fn instruction_evidence(
    bytes: &[u8],
    address: u64,
    instruction: &r5900::Instruction,
) -> TypedInstructionEvidence {
    TypedInstructionEvidence {
        address: format!("{address:#010x}"),
        bytes: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        canonical: r5900::format_instruction(instruction),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_loaded_address(
    source: &[u8],
    start_address: u64,
    upper_offset: usize,
    lower_offset: usize,
    register: GeneralRegister,
    expected_address: u64,
    role: &str,
) -> Result<Vec<TypedInstructionEvidence>> {
    let upper = decode_instruction(source, start_address, upper_offset)?;
    let upper_immediate = match upper.operands() {
        Operands::LoadUpper {
            destination,
            immediate,
        } if *destination == register => *immediate,
        _ => anyhow::bail!("{role} upper address instruction changed"),
    };
    let lower = decode_instruction(source, start_address, lower_offset)?;
    let lower_immediate = match lower.operands() {
        Operands::SignedImmediate {
            destination,
            source,
            immediate,
        } if *destination == register && *source == register => *immediate,
        _ => anyhow::bail!("{role} lower address instruction changed"),
    };
    ensure!(
        upper.form() == InstructionForm::Lui && lower.form() == InstructionForm::Addiu,
        "{role} address materialization is not lui/addiu"
    );
    let resolved_address =
        (u32::from(upper_immediate) << 16).wrapping_add_signed(i32::from(lower_immediate));
    ensure!(
        u64::from(resolved_address) == expected_address,
        "{role} resolved address changed"
    );
    Ok(vec![
        instruction_evidence(
            &source[upper_offset..upper_offset + 4],
            start_address + u64::try_from(upper_offset)?,
            &upper,
        ),
        instruction_evidence(
            &source[lower_offset..lower_offset + 4],
            start_address + u64::try_from(lower_offset)?,
            &lower,
        ),
    ])
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_loaded_memory_address(
    source: &[u8],
    start_address: u64,
    upper_offset: usize,
    memory_offset: usize,
    address_register: GeneralRegister,
    memory_form: InstructionForm,
    value_register: GeneralRegister,
    expected_address: u64,
    role: &str,
) -> Result<Vec<TypedInstructionEvidence>> {
    let upper = decode_instruction(source, start_address, upper_offset)?;
    let upper_immediate = match upper.operands() {
        Operands::LoadUpper {
            destination,
            immediate,
        } if *destination == address_register => *immediate,
        _ => anyhow::bail!("{role} upper address instruction changed"),
    };
    ensure!(
        upper.form() == InstructionForm::Lui,
        "{role} upper address instruction is not lui"
    );

    let memory = decode_instruction(source, start_address, memory_offset)?;
    let displacement = match memory.operands() {
        Operands::MemoryGeneral {
            register,
            base,
            offset,
        } if *register == value_register && *base == address_register => *offset,
        _ => anyhow::bail!("{role} memory instruction changed"),
    };
    ensure!(
        memory.form() == memory_form,
        "{role} memory instruction form changed"
    );

    let resolved_address =
        (u32::from(upper_immediate) << 16).wrapping_add_signed(i32::from(displacement));
    ensure!(
        u64::from(resolved_address) == expected_address,
        "{role} resolved address changed"
    );
    Ok(vec![
        instruction_evidence(
            &source[upper_offset..upper_offset + 4],
            start_address + u64::try_from(upper_offset)?,
            &upper,
        ),
        instruction_evidence(
            &source[memory_offset..memory_offset + 4],
            start_address + u64::try_from(memory_offset)?,
            &memory,
        ),
    ])
}

pub(crate) fn validate_jump_target(
    source: &[u8],
    start_address: u64,
    offset: usize,
    expected_target: u64,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == InstructionForm::Jal,
        "{role} call is not jal"
    );
    ensure!(
        matches!(instruction.operands(), Operands::Jump { target } if u64::from(*target) == expected_target),
        "{role} call target changed"
    );
    Ok(instruction_evidence(
        &source[offset..offset + 4],
        start_address + u64::try_from(offset)?,
        &instruction,
    ))
}

pub(crate) fn symbol_address(file: &object::File<'_>, name: &str) -> Result<u64> {
    let symbol = file
        .symbols()
        .find(|symbol| symbol.name().is_ok_and(|candidate| candidate == name))
        .with_context(|| format!("ELF symbol not found: {name}"))?;
    ensure!(symbol.is_definition(), "ELF symbol is not defined: {name}");
    Ok(symbol.address())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn require_signed_immediate(
    source: &[u8],
    start_address: u64,
    offset: usize,
    form: InstructionForm,
    destination: GeneralRegister,
    input: GeneralRegister,
    immediate: i16,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == form
            && matches!(
                instruction.operands(),
                Operands::SignedImmediate {
                    destination: actual_destination,
                    source: actual_source,
                    immediate: actual_immediate,
                } if *actual_destination == destination
                    && *actual_source == input
                    && *actual_immediate == immediate
            ),
        "{role} instruction changed"
    );
    Ok(instruction_evidence(
        &source[offset..offset + 4],
        start_address + u64::try_from(offset)?,
        &instruction,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn require_unsigned_immediate(
    source: &[u8],
    start_address: u64,
    offset: usize,
    form: InstructionForm,
    destination: GeneralRegister,
    input: GeneralRegister,
    immediate: u16,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == form
            && matches!(
                instruction.operands(),
                Operands::UnsignedImmediate {
                    destination: actual_destination,
                    source: actual_source,
                    immediate: actual_immediate,
                } if *actual_destination == destination
                    && *actual_source == input
                    && *actual_immediate == immediate
            ),
        "{role} instruction changed"
    );
    Ok(instruction_evidence(
        &source[offset..offset + 4],
        start_address + u64::try_from(offset)?,
        &instruction,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn require_shift(
    source: &[u8],
    start_address: u64,
    offset: usize,
    form: InstructionForm,
    destination: GeneralRegister,
    input: GeneralRegister,
    amount: u8,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == form
            && matches!(
                instruction.operands(),
                Operands::ShiftImmediate {
                    destination: actual_destination,
                    source: actual_source,
                    amount: actual_amount,
                } if *actual_destination == destination
                    && *actual_source == input
                    && actual_amount.value() == amount
            ),
        "{role} instruction changed"
    );
    Ok(instruction_evidence(
        &source[offset..offset + 4],
        start_address + u64::try_from(offset)?,
        &instruction,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn require_three_general(
    source: &[u8],
    start_address: u64,
    offset: usize,
    form: InstructionForm,
    destination: GeneralRegister,
    left: GeneralRegister,
    right: GeneralRegister,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == form
            && matches!(
                instruction.operands(),
                Operands::ThreeGeneral {
                    destination: actual_destination,
                    left: actual_left,
                    right: actual_right,
                } if *actual_destination == destination
                    && *actual_left == left
                    && *actual_right == right
            ),
        "{role} instruction changed"
    );
    Ok(instruction_evidence(
        &source[offset..offset + 4],
        start_address + u64::try_from(offset)?,
        &instruction,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn require_memory(
    source: &[u8],
    start_address: u64,
    offset: usize,
    form: InstructionForm,
    register: GeneralRegister,
    base: GeneralRegister,
    displacement: i16,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == form
            && matches!(
                instruction.operands(),
                Operands::MemoryGeneral {
                    register: actual_register,
                    base: actual_base,
                    offset: actual_displacement,
                } if *actual_register == register
                    && *actual_base == base
                    && *actual_displacement == displacement
            ),
        "{role} instruction changed"
    );
    Ok(instruction_evidence(
        &source[offset..offset + 4],
        start_address + u64::try_from(offset)?,
        &instruction,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn require_branch_two(
    source: &[u8],
    start_address: u64,
    offset: usize,
    form: InstructionForm,
    left: GeneralRegister,
    right: GeneralRegister,
    target: u64,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == form
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
        "{role} instruction changed"
    );
    Ok(instruction_evidence(
        &source[offset..offset + 4],
        start_address + u64::try_from(offset)?,
        &instruction,
    ))
}
