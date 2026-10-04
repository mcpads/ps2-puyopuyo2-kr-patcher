use super::catalog::*;
use super::*;

pub(super) fn instruction_word(source: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = source.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

pub(super) fn validate_actor_creation(
    source: &[u8],
    start_address: u64,
    spec: ActorCreationSpec,
    renderer_address: u64,
    renderer_symbol: &str,
    setter_address: u64,
) -> Result<Vec<TypedInstructionEvidence>> {
    let mut instructions = validate_loaded_address(
        source,
        start_address,
        spec.upper_offset,
        spec.lower_offset,
        GeneralRegister::A0,
        renderer_address,
        renderer_symbol,
    )?;
    instructions.push(validate_jump_target(
        source,
        start_address,
        spec.call_offset,
        setter_address,
        "message actor creation",
    )?);
    Ok(instructions)
}

pub(super) fn collect_message_store_offsets(
    source: &[u8],
    start_address: u64,
) -> Result<(BTreeSet<usize>, usize)> {
    ensure!(
        source.len().is_multiple_of(4),
        "consumer instruction bytes are not aligned"
    );
    let mut nonnull = BTreeSet::new();
    let mut nulls = 0_usize;
    for offset in (0..source.len()).step_by(4) {
        let instruction = decode_instruction(source, start_address, offset)?;
        if instruction.form() != InstructionForm::Sw {
            continue;
        }
        if let Operands::MemoryGeneral {
            register,
            offset: field_offset,
            ..
        } = instruction.operands()
            && *field_offset == MESSAGE_FIELD_OFFSET
        {
            if *register == GeneralRegister::ZERO {
                nulls += 1;
            } else {
                nonnull.insert(offset);
            }
        }
    }
    Ok((nonnull, nulls))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_loaded_address(
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
    let resolved =
        (u32::from(upper_immediate) << 16).wrapping_add_signed(i32::from(lower_immediate));
    ensure!(
        u64::from(resolved) == expected_address,
        "{role} resolved address changed"
    );
    Ok(vec![
        instruction_evidence(source, start_address, upper_offset, &upper),
        instruction_evidence(source, start_address, lower_offset, &lower),
    ])
}

pub(super) fn validate_jump_target(
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
        source,
        start_address,
        offset,
        &instruction,
    ))
}

pub(super) fn validate_message_store(
    source: &[u8],
    start_address: u64,
    offset: usize,
    value_register: GeneralRegister,
    base_register: GeneralRegister,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == InstructionForm::Sw,
        "message pointer store is not sw"
    );
    ensure!(
        matches!(
            instruction.operands(),
            Operands::MemoryGeneral { register, base, offset: field_offset }
                if *register == value_register
                    && *base == base_register
                    && *field_offset == MESSAGE_FIELD_OFFSET
        ),
        "message pointer store operands changed"
    );
    Ok(instruction_evidence(
        source,
        start_address,
        offset,
        &instruction,
    ))
}

pub(super) fn validate_tutorial_table_index_load(
    source: &[u8],
    start_address: u64,
    offset: usize,
) -> Result<TypedInstructionEvidence> {
    validate_memory_instruction(
        source,
        start_address,
        offset,
        InstructionForm::Lhu,
        GeneralRegister::A2,
        GeneralRegister::A2,
        4,
        "tutorial message-table index",
    )
}

pub(super) fn validate_tutorial_renderer(
    source: &[u8],
    start_address: u64,
    lookup_address: u64,
) -> Result<Vec<TypedInstructionEvidence>> {
    Ok(vec![
        validate_addiu(
            source,
            start_address,
            0xb8,
            GeneralRegister::V1,
            GeneralRegister::ZERO,
            10,
            "tutorial line-break byte",
        )?,
        validate_addiu(
            source,
            start_address,
            0xc4,
            GeneralRegister::S1,
            GeneralRegister::S1,
            1,
            "tutorial line-break input advance",
        )?,
        validate_addiu(
            source,
            start_address,
            0xd0,
            GeneralRegister::S3,
            GeneralRegister::S3,
            48,
            "tutorial line height",
        )?,
        validate_jump_target(
            source,
            start_address,
            0xe0,
            lookup_address,
            "tutorial font lookup",
        )?,
        validate_addiu(
            source,
            start_address,
            0x154,
            GeneralRegister::S1,
            GeneralRegister::S1,
            2,
            "tutorial Shift-JIS input advance",
        )?,
    ])
}

pub(super) fn validate_tutorial_font_lookup(
    source: &[u8],
    start_address: u64,
    font_table_address: u64,
) -> Result<Vec<TypedInstructionEvidence>> {
    let mut instructions = validate_loaded_address(
        source,
        start_address,
        0x98,
        0x9c,
        GeneralRegister::A3,
        font_table_address,
        TUTORIAL_FONT_TABLE_SYMBOL,
    )?;
    instructions.push(validate_shift_immediate(
        source,
        start_address,
        0xb8,
        InstructionForm::Srl,
        GeneralRegister::V1,
        GeneralRegister::T1,
        1,
        "tutorial font index",
    )?);
    instructions.push(validate_addiu(
        source,
        start_address,
        0xc8,
        GeneralRegister::A3,
        GeneralRegister::A3,
        2,
        "tutorial font-table advance",
    )?);
    instructions.push(validate_memory_instruction(
        source,
        start_address,
        0xcc,
        InstructionForm::Lb,
        GeneralRegister::V1,
        GeneralRegister::A3,
        0,
        "tutorial font-table terminator",
    )?);
    instructions.push(validate_addiu(
        source,
        start_address,
        0xd4,
        GeneralRegister::T1,
        GeneralRegister::T1,
        2,
        "tutorial font-table byte index",
    )?);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(instructions)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_addiu(
    source: &[u8],
    start_address: u64,
    offset: usize,
    destination: GeneralRegister,
    source_register: GeneralRegister,
    immediate: i16,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == InstructionForm::Addiu,
        "{role} is not addiu"
    );
    ensure!(
        matches!(
            instruction.operands(),
            Operands::SignedImmediate { destination: actual_destination, source, immediate: actual_immediate }
                if *actual_destination == destination
                    && *source == source_register
                    && *actual_immediate == immediate
        ),
        "{role} operands changed"
    );
    Ok(instruction_evidence(
        source,
        start_address,
        offset,
        &instruction,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_shift_immediate(
    source: &[u8],
    start_address: u64,
    offset: usize,
    form: InstructionForm,
    destination: GeneralRegister,
    source_register: GeneralRegister,
    amount: u8,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(instruction.form() == form, "{role} shift form changed");
    ensure!(
        matches!(
            instruction.operands(),
            Operands::ShiftImmediate { destination: actual_destination, source, amount: actual_amount }
                if *actual_destination == destination
                    && *source == source_register
                    && actual_amount.value() == amount
        ),
        "{role} operands changed"
    );
    Ok(instruction_evidence(
        source,
        start_address,
        offset,
        &instruction,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_memory_instruction(
    source: &[u8],
    start_address: u64,
    offset: usize,
    form: InstructionForm,
    register: GeneralRegister,
    base: GeneralRegister,
    memory_offset: i16,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(instruction.form() == form, "{role} memory form changed");
    ensure!(
        matches!(
            instruction.operands(),
            Operands::MemoryGeneral { register: actual_register, base: actual_base, offset }
                if *actual_register == register
                    && *actual_base == base
                    && *offset == memory_offset
        ),
        "{role} operands changed"
    );
    Ok(instruction_evidence(
        source,
        start_address,
        offset,
        &instruction,
    ))
}

pub(super) fn validate_table_lookup(
    source: &[u8],
    start_address: u64,
    spec: TableReferenceSpec,
) -> Result<Vec<TypedInstructionEvidence>> {
    let shift = decode_instruction(source, start_address, spec.shift_offset)?;
    ensure!(
        shift.form() == InstructionForm::Sll,
        "message table index is not shifted with sll"
    );
    ensure!(
        matches!(
            shift.operands(),
            Operands::ShiftImmediate { destination, source, amount }
                if *destination == spec.index_register
                    && *source == spec.index_source_register
                    && amount.value() == 2
        ),
        "message table index is no longer multiplied by four"
    );
    let add = decode_instruction(source, start_address, spec.add_offset)?;
    ensure!(
        add.form() == InstructionForm::Addu,
        "message table address is not addu"
    );
    ensure!(
        matches!(
            add.operands(),
            Operands::ThreeGeneral { destination, left, right }
                if *destination == spec.table_register
                    && *left == spec.table_register
                    && *right == spec.index_register
        ),
        "message table address operands changed"
    );
    let load = decode_instruction(source, start_address, spec.load_offset)?;
    ensure!(
        load.form() == InstructionForm::Lw,
        "message table entry is not loaded with lw"
    );
    ensure!(
        matches!(
            load.operands(),
            Operands::MemoryGeneral { register, base, offset }
                if *register == spec.message_register
                    && *base == spec.table_register
                    && *offset == 0
        ),
        "message table load operands changed"
    );
    Ok(vec![
        instruction_evidence(source, start_address, spec.shift_offset, &shift),
        instruction_evidence(source, start_address, spec.add_offset, &add),
        instruction_evidence(source, start_address, spec.load_offset, &load),
    ])
}

pub(super) fn validate_pointer_table(
    file: &object::File<'_>,
    spec: TableReferenceSpec,
    table_bytes: &[u8],
) -> Result<()> {
    ensure!(
        table_bytes.len() == spec.messages.len() * 4,
        "{} is not exactly {} 32-bit pointers",
        spec.table_symbol,
        spec.messages.len()
    );
    for (index, message) in spec.messages.iter().enumerate() {
        let pointer = u64::from(read_u32_at(table_bytes, index * 4)?);
        let (expected, _) = read_symbol_bytes(file, message.symbol)?;
        ensure!(
            pointer == expected,
            "{} pointer {index} no longer targets {}",
            spec.table_symbol,
            message.symbol
        );
    }
    Ok(())
}

pub(super) fn validate_control_dispatch(
    source: &[u8],
    start_address: u64,
) -> Result<Vec<TypedInstructionEvidence>> {
    let checks = [
        (
            0x68,
            GeneralRegister::V1,
            GeneralRegister::ZERO,
            0x0a_i16,
            "line break",
        ),
        (
            0x88,
            GeneralRegister::V1,
            GeneralRegister::ZERO,
            0x40_i16,
            "control prefix",
        ),
        (
            0x9c,
            GeneralRegister::V1,
            GeneralRegister::A0,
            0x41_i16,
            "button lower bound",
        ),
        (
            0xa4,
            GeneralRegister::AT,
            GeneralRegister::A0,
            0x5b_i16,
            "button upper bound",
        ),
        (
            0xb0,
            GeneralRegister::V0,
            GeneralRegister::ZERO,
            0x50_i16,
            "dynamic port control",
        ),
        (
            0x1a0,
            GeneralRegister::V1,
            GeneralRegister::ZERO,
            0x30_i16,
            "color control",
        ),
    ];
    checks
        .into_iter()
        .map(|(offset, destination, source_register, immediate, role)| {
            let instruction = decode_instruction(source, start_address, offset)?;
            ensure!(
                matches!(instruction.form(), InstructionForm::Addiu | InstructionForm::Slti),
                "{role} comparison setup changed form"
            );
            ensure!(
                matches!(
                    instruction.operands(),
                    Operands::SignedImmediate { destination: actual_destination, source: actual_source, immediate: actual_immediate }
                        if *actual_destination == destination
                            && *actual_source == source_register
                            && *actual_immediate == immediate
                ),
                "{role} comparison setup changed operands"
            );
            Ok(instruction_evidence(source, start_address, offset, &instruction))
        })
        .collect()
}

pub(super) fn decode_instruction(
    source: &[u8],
    start_address: u64,
    offset: usize,
) -> Result<r5900::Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .context("message consumer instruction offset is out of range")?;
    let address = start_address + u64::try_from(offset)?;
    validate_r5900_instruction(bytes, address)
        .with_context(|| format!("failed to validate message consumer at {address:#x}"))
}

pub(super) fn instruction_evidence(
    source: &[u8],
    start_address: u64,
    offset: usize,
    instruction: &r5900::Instruction,
) -> TypedInstructionEvidence {
    TypedInstructionEvidence {
        address: format!("{:#010x}", start_address + offset as u64),
        bytes: hex(&source[offset..offset + 4]),
        canonical: r5900::format_instruction(instruction),
    }
}
