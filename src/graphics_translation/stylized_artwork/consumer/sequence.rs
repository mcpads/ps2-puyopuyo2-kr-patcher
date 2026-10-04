use super::*;

pub(in crate::graphics_translation::stylized_artwork) fn find_sequence_command<'a>(
    symbol: &str,
    source: &'a [u8],
    command_sizes: &[usize],
    expected_offset: usize,
) -> Result<&'a [u8]> {
    let mut offset = 0;
    let mut selected = None;
    while offset < source.len() {
        let opcode_bytes = source
            .get(offset..offset + 2)
            .with_context(|| format!("{symbol} ends inside a sequence opcode"))?;
        let opcode = usize::from(u16::from_le_bytes(opcode_bytes.try_into()?));
        let words = *command_sizes
            .get(opcode)
            .with_context(|| format!("{symbol} uses opcode {opcode:#x} outside SeqComSize"))?;
        ensure!(words > 0, "{symbol} uses zero-sized opcode {opcode:#x}");
        let byte_length = words
            .checked_mul(2)
            .context("stylized artwork sequence command length overflow")?;
        let command = source
            .get(offset..offset + byte_length)
            .with_context(|| format!("{symbol} ends inside opcode {opcode:#x}"))?;
        if offset == expected_offset {
            selected = Some(command);
        }
        offset += byte_length;
    }
    ensure!(
        offset == source.len(),
        "{symbol} sequence parsing did not end exactly"
    );
    selected.with_context(|| {
        format!("{symbol} has no command boundary at byte offset {expected_offset:#x}")
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_update_actor_sequence_decoder(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkSequenceConsumer,
) -> Result<Vec<TypedInstructionEvidence>> {
    ensure!(
        consumer.sequence_opcode >= COMMAND_DISPATCH_FIRST_OPCODE
            && consumer.sequence_opcode < COMMAND_DISPATCH_FIRST_OPCODE + COMMAND_DISPATCH_ENTRIES,
        "stylized artwork opcode is outside the SeqCommand dispatch table"
    );
    ensure!(
        disassemble_symbol(executable, SEQUENCE_COMMAND_SYMBOL)?
            .instructions
            .len()
            == SEQUENCE_COMMAND_INSTRUCTIONS,
        "SeqCommand instruction population changed"
    );
    let (address, source) = read_symbol_bytes(file, SEQUENCE_COMMAND_SYMBOL)?;
    let mut evidence = validate_loaded_address(
        &source,
        address,
        0x5c,
        0x64,
        GeneralRegister::A1,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        "sequence command dispatch table",
    )?;
    evidence.extend([
        require_memory(
            &source,
            address,
            0x48,
            InstructionForm::Lw,
            GeneralRegister::V1,
            GeneralRegister::S1,
            556,
            "sequence command pointer",
        )?,
        require_memory(
            &source,
            address,
            0x4c,
            InstructionForm::Lhu,
            GeneralRegister::V0,
            GeneralRegister::V1,
            0,
            "sequence opcode",
        )?,
        require_signed_immediate(
            &source,
            address,
            0x50,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::V0,
            -(COMMAND_DISPATCH_FIRST_OPCODE as i16),
            "sequence dispatch index",
        )?,
        require_signed_immediate(
            &source,
            address,
            0x54,
            InstructionForm::Sltiu,
            GeneralRegister::AT,
            GeneralRegister::A0,
            COMMAND_DISPATCH_ENTRIES as i16,
            "sequence dispatch range",
        )?,
        require_shift(
            &source,
            address,
            0x60,
            InstructionForm::Sll,
            GeneralRegister::A0,
            GeneralRegister::A0,
            2,
            "sequence dispatch pointer scale",
        )?,
        require_three_general(
            &source,
            address,
            0x68,
            InstructionForm::Addu,
            GeneralRegister::A0,
            GeneralRegister::A0,
            GeneralRegister::A1,
            "sequence dispatch pointer",
        )?,
        require_memory(
            &source,
            address,
            0x6c,
            InstructionForm::Lw,
            GeneralRegister::A0,
            GeneralRegister::A0,
            0,
            "sequence dispatch target",
        )?,
    ]);
    let dispatch = read_address_range(
        file,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        u64::try_from(COMMAND_DISPATCH_ENTRIES * 4)?,
    )?;
    let entry_offset = (consumer.sequence_opcode - COMMAND_DISPATCH_FIRST_OPCODE) * 4;
    let case_address = u64::from(u32::from_le_bytes(
        dispatch[entry_offset..entry_offset + 4]
            .try_into()
            .expect("fixed dispatch entry width"),
    ));
    ensure!(
        case_address == consumer.sequence_case_address,
        "stylized artwork opcode dispatch target changed"
    );
    let case_offset = usize::try_from(
        consumer
            .sequence_case_address
            .checked_sub(address)
            .context("stylized artwork sequence case precedes SeqCommand")?,
    )?;
    evidence.extend([
        require_memory(
            &source,
            address,
            case_offset,
            InstructionForm::Lhu,
            GeneralRegister::A0,
            GeneralRegister::V1,
            2,
            "stylized actor index",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x0c,
            InstructionForm::Lh,
            GeneralRegister::V1,
            GeneralRegister::V1,
            4,
            "stylized actor x position",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x40,
            InstructionForm::Lh,
            GeneralRegister::V1,
            GeneralRegister::V1,
            6,
            "stylized actor y position",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x68,
            InstructionForm::Lhu,
            GeneralRegister::A1,
            GeneralRegister::V1,
            8,
            "stylized actor depth",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x98,
            InstructionForm::Lhu,
            GeneralRegister::A1,
            GeneralRegister::V1,
            10,
            "stylized pattern index",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0xb4,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::A1,
            1,
            "stylized pattern index times two",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0xb8,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            GeneralRegister::A1,
            "stylized pattern index times three",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0xbc,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            3,
            "stylized pattern record byte stride",
        )?,
        validate_jump_target(
            &source,
            address,
            case_offset + 0xc0,
            symbol_address(file, &consumer.pattern_setter_symbol)?,
            "stylized pattern setter",
        )?,
    ]);
    evidence.extend(validate_loaded_address(
        &source,
        address,
        0x161c,
        0x1620,
        GeneralRegister::V0,
        symbol_address(file, SEQUENCE_COMMAND_SIZE_SYMBOL)?,
        "sequence command size table",
    )?);
    evidence.extend([
        require_memory(
            &source,
            address,
            0x1624,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::A0,
            0,
            "sequence command size opcode",
        )?,
        require_shift(
            &source,
            address,
            0x1628,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            1,
            "sequence command size index",
        )?,
        require_memory(
            &source,
            address,
            0x1630,
            InstructionForm::Lh,
            GeneralRegister::V0,
            GeneralRegister::V0,
            0,
            "sequence command word length",
        )?,
        require_shift(
            &source,
            address,
            0x1634,
            InstructionForm::Sll,
            GeneralRegister::V0,
            GeneralRegister::V0,
            1,
            "sequence command byte length",
        )?,
    ]);
    evidence.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(evidence)
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_set_pattern_sequence_decoder(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkSequenceConsumer,
) -> Result<Vec<TypedInstructionEvidence>> {
    ensure!(
        (COMMAND_DISPATCH_FIRST_OPCODE..COMMAND_DISPATCH_FIRST_OPCODE + COMMAND_DISPATCH_ENTRIES)
            .contains(&consumer.sequence_opcode),
        "pattern-selection opcode is outside the SeqCommand dispatch table"
    );
    ensure!(
        disassemble_symbol(executable, SEQUENCE_COMMAND_SYMBOL)?
            .instructions
            .len()
            == SEQUENCE_COMMAND_INSTRUCTIONS,
        "SeqCommand instruction population changed"
    );

    let (address, source) = read_symbol_bytes(file, SEQUENCE_COMMAND_SYMBOL)?;
    let dispatch = read_address_range(
        file,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        u64::try_from(COMMAND_DISPATCH_ENTRIES * 4)?,
    )?;
    let entry_offset = (consumer.sequence_opcode - COMMAND_DISPATCH_FIRST_OPCODE) * 4;
    let case_address = u64::from(u32::from_le_bytes(
        dispatch[entry_offset..entry_offset + 4]
            .try_into()
            .expect("fixed dispatch entry width"),
    ));
    ensure!(
        case_address == consumer.sequence_case_address,
        "pattern-selection opcode dispatch target changed"
    );
    let case_offset = usize::try_from(
        consumer
            .sequence_case_address
            .checked_sub(address)
            .context("pattern-selection sequence case precedes SeqCommand")?,
    )?;

    let mut evidence = validate_loaded_address(
        &source,
        address,
        0x5c,
        0x64,
        GeneralRegister::A1,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        "pattern-selection command dispatch table",
    )?;
    evidence.extend([
        require_memory(
            &source,
            address,
            case_offset,
            InstructionForm::Lhu,
            GeneralRegister::A0,
            GeneralRegister::V1,
            2,
            "pattern-selection actor index",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x04,
            InstructionForm::Lhu,
            GeneralRegister::A1,
            GeneralRegister::V1,
            4,
            "pattern-selection pattern index",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x0c,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::A0,
            2,
            "pattern-selection actor pointer index",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x10,
            InstructionForm::Addu,
            GeneralRegister::A0,
            GeneralRegister::S1,
            GeneralRegister::V1,
            "pattern-selection actor pointer address",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x20,
            InstructionForm::Lw,
            GeneralRegister::A0,
            GeneralRegister::A0,
            32,
            "pattern-selection actor pointer",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x24,
            InstructionForm::Lw,
            GeneralRegister::V0,
            GeneralRegister::S1,
            544,
            "pattern-selection table pointer",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x2c,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::A1,
            1,
            "pattern-selection index times two",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x30,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            GeneralRegister::A1,
            "pattern-selection index times three",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x34,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            3,
            "pattern-selection record byte stride",
        )?,
        validate_jump_target(
            &source,
            address,
            case_offset + 0x38,
            symbol_address(file, &consumer.pattern_setter_symbol)?,
            "pattern-selection pattern setter",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x3c,
            InstructionForm::Addu,
            GeneralRegister::A1,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "pattern-selection record pointer",
        )?,
    ]);
    evidence.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(evidence)
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_animation_sequence_decoder(
    file: &object::File<'_>,
    executable: &[u8],
    route: AnimationSequenceDecoderRoute<'_>,
) -> Result<Vec<TypedInstructionEvidence>> {
    ensure!(
        (COMMAND_DISPATCH_FIRST_OPCODE..COMMAND_DISPATCH_FIRST_OPCODE + COMMAND_DISPATCH_ENTRIES)
            .contains(&route.animation_opcode),
        "animation opcode is outside the SeqCommand dispatch table"
    );
    ensure!(
        disassemble_symbol(executable, SEQUENCE_COMMAND_SYMBOL)?
            .instructions
            .len()
            == SEQUENCE_COMMAND_INSTRUCTIONS,
        "SeqCommand instruction population changed"
    );
    let (address, source) = read_symbol_bytes(file, SEQUENCE_COMMAND_SYMBOL)?;
    let mut evidence = validate_loaded_address(
        &source,
        address,
        0x5c,
        0x64,
        GeneralRegister::A1,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        "animation command dispatch table",
    )?;
    let dispatch = read_address_range(
        file,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        u64::try_from(COMMAND_DISPATCH_ENTRIES * 4)?,
    )?;
    let entry_offset = (route.animation_opcode - COMMAND_DISPATCH_FIRST_OPCODE) * 4;
    let case_address = u64::from(u32::from_le_bytes(
        dispatch[entry_offset..entry_offset + 4]
            .try_into()
            .expect("fixed dispatch entry width"),
    ));
    ensure!(
        case_address == route.animation_case_address,
        "animation opcode dispatch target changed"
    );
    let case_offset = usize::try_from(
        route
            .animation_case_address
            .checked_sub(address)
            .context("animation sequence case precedes SeqCommand")?,
    )?;
    ensure!(
        case_offset == 0x8b0,
        "animation sequence case offset changed"
    );
    evidence.extend([
        require_memory(
            &source,
            address,
            case_offset,
            InstructionForm::Lhu,
            GeneralRegister::V0,
            GeneralRegister::V1,
            2,
            "animation actor index",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x04,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::S1,
            0x220,
            "animation pattern table pointer",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x08,
            InstructionForm::Sll,
            GeneralRegister::V0,
            GeneralRegister::V0,
            2,
            "animation actor index scale",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x0c,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::S1,
            GeneralRegister::V0,
            "animation actor table entry",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x10,
            InstructionForm::Lw,
            GeneralRegister::S2,
            GeneralRegister::V0,
            0x20,
            "animation actor pointer",
        )?,
        validate_jump_target(
            &source,
            address,
            case_offset + 0x14,
            symbol_address(file, route.cell_initializer_symbol)?,
            "animation cell initializer",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x1c,
            InstructionForm::Lw,
            GeneralRegister::V1,
            GeneralRegister::S1,
            0x22c,
            "animation sequence command pointer",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x20,
            InstructionForm::Lw,
            GeneralRegister::V0,
            GeneralRegister::S1,
            0x224,
            "animation pointer-table base",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x24,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            4,
            "animation table index",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x28,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "animation table index scale",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x2c,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "animation pointer-table entry",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x30,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "selected animation table pointer",
        )?,
        validate_jump_target(
            &source,
            address,
            case_offset + 0x34,
            symbol_address(file, route.animation_initializer_symbol)?,
            "animation initializer",
        )?,
    ]);
    evidence.extend(validate_loaded_address(
        &source,
        address,
        0x161c,
        0x1620,
        GeneralRegister::V0,
        symbol_address(file, SEQUENCE_COMMAND_SIZE_SYMBOL)?,
        "animation command size table",
    )?);
    evidence.extend([
        require_memory(
            &source,
            address,
            0x1624,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::A0,
            0,
            "animation command size opcode",
        )?,
        require_shift(
            &source,
            address,
            0x1628,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            1,
            "animation command size index",
        )?,
        require_memory(
            &source,
            address,
            0x1630,
            InstructionForm::Lh,
            GeneralRegister::V0,
            GeneralRegister::V0,
            0,
            "animation command word length",
        )?,
        require_shift(
            &source,
            address,
            0x1634,
            InstructionForm::Sll,
            GeneralRegister::V0,
            GeneralRegister::V0,
            1,
            "animation command byte length",
        )?,
    ]);
    evidence.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(evidence)
}

#[derive(Clone, Copy)]
pub(in crate::graphics_translation::stylized_artwork) struct AnimationSequenceDecoderRoute<'a> {
    pub(in crate::graphics_translation::stylized_artwork) animation_opcode: usize,
    pub(in crate::graphics_translation::stylized_artwork) animation_case_address: u64,
    pub(in crate::graphics_translation::stylized_artwork) cell_initializer_symbol: &'a str,
    pub(in crate::graphics_translation::stylized_artwork) animation_initializer_symbol: &'a str,
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_title_sequence_decoder(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkTitleBrandingConsumer,
) -> Result<Vec<TypedInstructionEvidence>> {
    validate_create_actor_pattern_decoder(
        file,
        executable,
        consumer.sequence_opcode,
        consumer.sequence_case_address,
        &consumer.pattern_setter_symbol,
    )
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_create_child_actor_pattern_decoder(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkSequenceConsumer,
    parent_create_opcode: usize,
    parent_create_case_address: u64,
) -> Result<Vec<TypedInstructionEvidence>> {
    ensure!(
        (COMMAND_DISPATCH_FIRST_OPCODE..COMMAND_DISPATCH_FIRST_OPCODE + COMMAND_DISPATCH_ENTRIES)
            .contains(&consumer.sequence_opcode),
        "create-child-actor pattern opcode is outside the SeqCommand dispatch table"
    );
    ensure!(
        disassemble_symbol(executable, SEQUENCE_COMMAND_SYMBOL)?
            .instructions
            .len()
            == SEQUENCE_COMMAND_INSTRUCTIONS,
        "SeqCommand instruction population changed"
    );

    let mut evidence = validate_create_actor_pattern_decoder(
        file,
        executable,
        parent_create_opcode,
        parent_create_case_address,
        &consumer.pattern_setter_symbol,
    )?;
    let (address, source) = read_symbol_bytes(file, SEQUENCE_COMMAND_SYMBOL)?;
    let dispatch = read_address_range(
        file,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        u64::try_from(COMMAND_DISPATCH_ENTRIES * 4)?,
    )?;
    let entry_offset = (consumer.sequence_opcode - COMMAND_DISPATCH_FIRST_OPCODE) * 4;
    let case_address = u64::from(u32::from_le_bytes(
        dispatch[entry_offset..entry_offset + 4]
            .try_into()
            .expect("fixed dispatch entry width"),
    ));
    ensure!(
        case_address == consumer.sequence_case_address,
        "create-child-actor pattern opcode dispatch target changed"
    );
    let case_offset = usize::try_from(
        consumer
            .sequence_case_address
            .checked_sub(address)
            .context("create-child-actor pattern sequence case precedes SeqCommand")?,
    )?;
    ensure!(
        case_offset == 0x958,
        "create-child-actor pattern sequence case offset changed"
    );

    evidence.extend([
        require_memory(
            &source,
            address,
            case_offset,
            InstructionForm::Lhu,
            GeneralRegister::A0,
            GeneralRegister::V1,
            6,
            "child actor parent index",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x04,
            InstructionForm::Lhu,
            GeneralRegister::S3,
            GeneralRegister::V1,
            4,
            "child actor flags",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x10,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::A0,
            2,
            "child actor parent index scale",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x14,
            InstructionForm::Addu,
            GeneralRegister::A0,
            GeneralRegister::S1,
            GeneralRegister::V1,
            "child actor parent table entry",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x18,
            InstructionForm::Lw,
            GeneralRegister::S4,
            GeneralRegister::A0,
            32,
            "child actor parent pointer",
        )?,
        validate_jump_target(
            &source,
            address,
            case_offset + 0x2c,
            symbol_address(file, "Act_SetBottom")?,
            "child actor creator",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x40,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "created child actor index",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x44,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "created child actor index scale",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x48,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::S1,
            GeneralRegister::V1,
            "created child actor table entry",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x4c,
            InstructionForm::Sw,
            GeneralRegister::S2,
            GeneralRegister::V1,
            32,
            "created child actor table store",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x54,
            InstructionForm::Lw,
            GeneralRegister::V1,
            GeneralRegister::S4,
            100,
            "child actor parent x position",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x58,
            InstructionForm::Lh,
            GeneralRegister::A1,
            GeneralRegister::A1,
            8,
            "child actor relative x position",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x5c,
            InstructionForm::Sll,
            GeneralRegister::A1,
            GeneralRegister::A1,
            16,
            "child actor relative x fixed-point conversion",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x60,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            GeneralRegister::A1,
            "child actor derived x position",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x64,
            InstructionForm::Sw,
            GeneralRegister::V1,
            GeneralRegister::V0,
            100,
            "child actor x position store",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x6c,
            InstructionForm::Lw,
            GeneralRegister::V1,
            GeneralRegister::S4,
            104,
            "child actor parent y position",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x70,
            InstructionForm::Lh,
            GeneralRegister::A1,
            GeneralRegister::A1,
            10,
            "child actor relative y position",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x74,
            InstructionForm::Sll,
            GeneralRegister::A1,
            GeneralRegister::A1,
            16,
            "child actor relative y fixed-point conversion",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x78,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            GeneralRegister::A1,
            "child actor derived y position",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x7c,
            InstructionForm::Sw,
            GeneralRegister::V1,
            GeneralRegister::V0,
            104,
            "child actor y position store",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x84,
            InstructionForm::Lw,
            GeneralRegister::V1,
            GeneralRegister::S4,
            108,
            "child actor parent depth",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x88,
            InstructionForm::Lh,
            GeneralRegister::A1,
            GeneralRegister::A1,
            12,
            "child actor relative depth",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x8c,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            GeneralRegister::A1,
            "child actor derived depth",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x90,
            InstructionForm::Sw,
            GeneralRegister::V1,
            GeneralRegister::V0,
            108,
            "child actor depth store",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x98,
            InstructionForm::Lw,
            GeneralRegister::V0,
            GeneralRegister::S1,
            544,
            "child actor pattern table pointer",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x9c,
            InstructionForm::Lhu,
            GeneralRegister::A1,
            GeneralRegister::V1,
            14,
            "child actor pattern index",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0xa0,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::A1,
            1,
            "child actor pattern index times two",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0xa4,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            GeneralRegister::A1,
            "child actor pattern index times three",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0xa8,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            3,
            "child actor pattern record byte stride",
        )?,
        validate_jump_target(
            &source,
            address,
            case_offset + 0xac,
            symbol_address(file, &consumer.pattern_setter_symbol)?,
            "child actor pattern setter",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0xb0,
            InstructionForm::Addu,
            GeneralRegister::A1,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "child actor pattern record address",
        )?,
    ]);
    evidence.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(evidence)
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_create_actor_pattern_decoder(
    file: &object::File<'_>,
    executable: &[u8],
    sequence_opcode: usize,
    sequence_case_address: u64,
    pattern_setter_symbol: &str,
) -> Result<Vec<TypedInstructionEvidence>> {
    ensure!(
        (COMMAND_DISPATCH_FIRST_OPCODE..COMMAND_DISPATCH_FIRST_OPCODE + COMMAND_DISPATCH_ENTRIES)
            .contains(&sequence_opcode),
        "create-actor pattern opcode is outside the SeqCommand dispatch table"
    );
    ensure!(
        disassemble_symbol(executable, SEQUENCE_COMMAND_SYMBOL)?
            .instructions
            .len()
            == SEQUENCE_COMMAND_INSTRUCTIONS,
        "SeqCommand instruction population changed"
    );
    let (address, source) = read_symbol_bytes(file, SEQUENCE_COMMAND_SYMBOL)?;
    let mut evidence = validate_loaded_address(
        &source,
        address,
        0x5c,
        0x64,
        GeneralRegister::A1,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        "create-actor pattern command dispatch table",
    )?;
    evidence.extend([
        require_memory(
            &source,
            address,
            0x48,
            InstructionForm::Lw,
            GeneralRegister::V1,
            GeneralRegister::S1,
            556,
            "create-actor pattern command pointer",
        )?,
        require_memory(
            &source,
            address,
            0x4c,
            InstructionForm::Lhu,
            GeneralRegister::V0,
            GeneralRegister::V1,
            0,
            "create-actor pattern opcode",
        )?,
        require_signed_immediate(
            &source,
            address,
            0x50,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::V0,
            -(COMMAND_DISPATCH_FIRST_OPCODE as i16),
            "create-actor pattern dispatch index",
        )?,
        require_signed_immediate(
            &source,
            address,
            0x54,
            InstructionForm::Sltiu,
            GeneralRegister::AT,
            GeneralRegister::A0,
            COMMAND_DISPATCH_ENTRIES as i16,
            "create-actor pattern dispatch range",
        )?,
        require_shift(
            &source,
            address,
            0x60,
            InstructionForm::Sll,
            GeneralRegister::A0,
            GeneralRegister::A0,
            2,
            "create-actor pattern dispatch pointer scale",
        )?,
        require_three_general(
            &source,
            address,
            0x68,
            InstructionForm::Addu,
            GeneralRegister::A0,
            GeneralRegister::A0,
            GeneralRegister::A1,
            "create-actor pattern dispatch pointer",
        )?,
        require_memory(
            &source,
            address,
            0x6c,
            InstructionForm::Lw,
            GeneralRegister::A0,
            GeneralRegister::A0,
            0,
            "create-actor pattern dispatch target",
        )?,
    ]);

    let dispatch = read_address_range(
        file,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        u64::try_from(COMMAND_DISPATCH_ENTRIES * 4)?,
    )?;
    let entry_offset = (sequence_opcode - COMMAND_DISPATCH_FIRST_OPCODE) * 4;
    let case_address = u64::from(u32::from_le_bytes(
        dispatch[entry_offset..entry_offset + 4]
            .try_into()
            .expect("fixed dispatch entry width"),
    ));
    ensure!(
        case_address == sequence_case_address,
        "create-actor pattern opcode dispatch target changed"
    );
    let case_offset = usize::try_from(
        sequence_case_address
            .checked_sub(address)
            .context("create-actor pattern sequence case precedes SeqCommand")?,
    )?;
    ensure!(
        case_offset == 0x78,
        "create-actor pattern sequence case offset changed"
    );
    evidence.extend([
        require_memory(
            &source,
            address,
            case_offset,
            InstructionForm::Lhu,
            GeneralRegister::S2,
            GeneralRegister::V1,
            4,
            "create-actor flags",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x30,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "created actor index",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x34,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "created actor index scale",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x38,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::S1,
            GeneralRegister::V1,
            "created actor table entry",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x3c,
            InstructionForm::Sw,
            GeneralRegister::S3,
            GeneralRegister::V1,
            32,
            "created actor table store",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x48,
            InstructionForm::Lhu,
            GeneralRegister::A1,
            GeneralRegister::A1,
            6,
            "created actor x position",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x58,
            InstructionForm::Lhu,
            GeneralRegister::A1,
            GeneralRegister::A1,
            8,
            "created actor y position",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x68,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            10,
            "created actor depth",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x74,
            InstructionForm::Lw,
            GeneralRegister::V0,
            GeneralRegister::S1,
            544,
            "created actor pattern table pointer",
        )?,
        require_memory(
            &source,
            address,
            case_offset + 0x78,
            InstructionForm::Lhu,
            GeneralRegister::A1,
            GeneralRegister::V1,
            12,
            "created actor pattern index",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x7c,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::A1,
            1,
            "created actor pattern index times two",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x80,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            GeneralRegister::A1,
            "created actor pattern index times three",
        )?,
        require_shift(
            &source,
            address,
            case_offset + 0x84,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            3,
            "created actor pattern record byte stride",
        )?,
        validate_jump_target(
            &source,
            address,
            case_offset + 0x88,
            symbol_address(file, pattern_setter_symbol)?,
            "create-actor pattern setter",
        )?,
        require_three_general(
            &source,
            address,
            case_offset + 0x8c,
            InstructionForm::Addu,
            GeneralRegister::A1,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "created actor pattern record address",
        )?,
    ]);
    evidence.extend(validate_loaded_address(
        &source,
        address,
        0x161c,
        0x1620,
        GeneralRegister::V0,
        symbol_address(file, SEQUENCE_COMMAND_SIZE_SYMBOL)?,
        "create-actor pattern command size table",
    )?);
    evidence.extend([
        require_memory(
            &source,
            address,
            0x1624,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::A0,
            0,
            "create-actor pattern command size opcode",
        )?,
        require_shift(
            &source,
            address,
            0x1628,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            1,
            "create-actor pattern command size index",
        )?,
        require_memory(
            &source,
            address,
            0x1630,
            InstructionForm::Lh,
            GeneralRegister::V0,
            GeneralRegister::V0,
            0,
            "title sequence command word length",
        )?,
        require_shift(
            &source,
            address,
            0x1634,
            InstructionForm::Sll,
            GeneralRegister::V0,
            GeneralRegister::V0,
            1,
            "title sequence command byte length",
        )?,
    ]);
    evidence.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(evidence)
}
