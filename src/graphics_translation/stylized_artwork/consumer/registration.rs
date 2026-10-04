use super::*;

#[derive(Clone, Copy)]
pub(in crate::graphics_translation::stylized_artwork) enum StylizedArtworkBattleRegistrationRouteRef<
    'a,
> {
    Sequential(&'a StylizedArtworkSequentialRegistrationRoute),
    Indexed(&'a StylizedArtworkIndexedRegistrationRoute),
    CharacterFace(&'a StylizedArtworkCharacterFaceRegistrationRoute),
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_sequential_registration_route(
    file: &object::File<'_>,
    executable: &[u8],
    route: &StylizedArtworkSequentialRegistrationRoute,
) -> Result<StylizedArtworkSequentialRegistrationValidation> {
    ensure!(
        route.loader_actor_symbol == "Puyo_Toko_Init"
            && route.registration_actor_symbol == "Puyo_Toko_Run"
            && route.loader_actor_instruction_count == 34
            && route.registration_actor_instruction_count == 149
            && !route.load_table_symbols.is_empty()
            && route.texture_slot >= route.image_load_index,
        "battle sequential registration route header changed"
    );
    let unique_load_tables = route
        .load_table_symbols
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    ensure!(
        unique_load_tables.len() == route.load_table_symbols.len(),
        "battle sequential registration route repeats a load table"
    );

    for symbol in &route.load_table_symbols {
        let (_, load_bytes) = read_symbol_bytes(file, symbol)?;
        let loads = parse_load_records(&load_bytes)?;
        let images = loads
            .iter()
            .filter(|record| record.operation == 1)
            .collect::<Vec<_>>();
        let palettes = loads
            .iter()
            .filter(|record| record.operation == 2)
            .collect::<Vec<_>>();
        ensure!(
            images
                .get(route.image_load_index)
                .is_some_and(|record| { record.file_record == route.image_file_record })
                && palettes
                    .get(route.palette_slot)
                    .is_some_and(|record| { record.file_record == route.palette_file_record }),
            "battle sequential load-table route changed: {symbol}"
        );
    }

    let (_, predecessor_bytes) = read_symbol_bytes(file, &route.predecessor_load_table_symbol)?;
    let predecessor_loads = parse_load_records(&predecessor_bytes)?;
    let predecessor_image_count = predecessor_loads
        .iter()
        .filter(|record| record.operation == 1)
        .count();
    ensure!(
        route.texture_slot == predecessor_image_count + route.image_load_index,
        "battle texture slot no longer follows the predecessor image registrations"
    );

    let (_, pointer_bytes) = read_symbol_bytes(file, &route.pointer_table_symbol)?;
    ensure!(
        pointer_bytes.len() == route.load_table_symbols.len() * 4,
        "battle load-table pointer table size changed"
    );
    let pointer_addresses = pointer_bytes
        .chunks_exact(4)
        .map(|chunk| {
            u64::from(u32::from_le_bytes(
                chunk.try_into().expect("four-byte chunk"),
            ))
        })
        .collect::<Vec<_>>();
    for (symbol, pointer_address) in route
        .load_table_symbols
        .iter()
        .zip(pointer_addresses.iter().copied())
    {
        ensure!(
            symbol_address(file, symbol)? == pointer_address,
            "battle load-table pointer changed: {symbol}"
        );
    }

    ensure!(
        disassemble_symbol(executable, &route.loader_actor_symbol)?
            .instructions
            .len()
            == route.loader_actor_instruction_count,
        "battle texture loader instruction population changed"
    );
    let (loader_address, loader_bytes) = read_symbol_bytes(file, &route.loader_actor_symbol)?;
    let mut typed_loader_instructions = validate_loaded_address(
        &loader_bytes,
        loader_address,
        0x04,
        0x14,
        GeneralRegister::A1,
        symbol_address(file, &route.predecessor_load_table_symbol)?,
        "battle predecessor load-table address",
    )?;
    typed_loader_instructions.extend(validate_loaded_address(
        &loader_bytes,
        loader_address,
        0x1c,
        0x24,
        GeneralRegister::V0,
        symbol_address(file, &route.pointer_table_symbol)?,
        "battle load-table pointer address",
    )?);
    typed_loader_instructions.extend([
        require_signed_immediate(
            &loader_bytes,
            loader_address,
            0x0c,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            1,
            "battle predecessor file identifier",
        )?,
        validate_jump_target(
            &loader_bytes,
            loader_address,
            0x10,
            symbol_address(file, "aqCdvd_ReadFile")?,
            "battle predecessor load call",
        )?,
        require_memory(
            &loader_bytes,
            loader_address,
            0x20,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_380,
            "battle load-table selector",
        )?,
        require_shift(
            &loader_bytes,
            loader_address,
            0x28,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "battle load-table selector scale",
        )?,
        require_three_general(
            &loader_bytes,
            loader_address,
            0x2c,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "battle selected load-table pointer",
        )?,
        require_memory(
            &loader_bytes,
            loader_address,
            0x30,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "battle selected load table",
        )?,
        validate_jump_target(
            &loader_bytes,
            loader_address,
            0x34,
            symbol_address(file, "aqCdvd_ReadFile")?,
            "battle selected-table load call",
        )?,
        require_signed_immediate(
            &loader_bytes,
            loader_address,
            0x38,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            1,
            "battle selected-table file identifier",
        )?,
    ]);
    typed_loader_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    ensure!(
        disassemble_symbol(executable, &route.registration_actor_symbol)?
            .instructions
            .len()
            == route.registration_actor_instruction_count,
        "battle texture registration actor instruction population changed"
    );
    let (registration_address, registration_bytes) =
        read_symbol_bytes(file, &route.registration_actor_symbol)?;
    let mut typed_registration_instructions = validate_loaded_address(
        &registration_bytes,
        registration_address,
        0x58,
        0x64,
        GeneralRegister::A1,
        symbol_address(file, &route.predecessor_load_table_symbol)?,
        "battle predecessor registration table",
    )?;
    typed_registration_instructions.extend(validate_loaded_address(
        &registration_bytes,
        registration_address,
        0x6c,
        0x74,
        GeneralRegister::V0,
        symbol_address(file, &route.pointer_table_symbol)?,
        "battle registration pointer table",
    )?);
    typed_registration_instructions.extend([
        validate_jump_target(
            &registration_bytes,
            registration_address,
            0x50,
            symbol_address(file, "ActRegList_Init")?,
            "battle registration initializer",
        )?,
        require_signed_immediate(
            &registration_bytes,
            registration_address,
            0x5c,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            1,
            "battle predecessor registration file identifier",
        )?,
        validate_jump_target(
            &registration_bytes,
            registration_address,
            0x60,
            symbol_address(file, "ActRegList_Regist2")?,
            "battle predecessor registration call",
        )?,
        require_memory(
            &registration_bytes,
            registration_address,
            0x70,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_380,
            "battle registration table selector",
        )?,
        require_shift(
            &registration_bytes,
            registration_address,
            0x78,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "battle registration selector scale",
        )?,
        require_three_general(
            &registration_bytes,
            registration_address,
            0x7c,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "battle selected registration pointer",
        )?,
        require_memory(
            &registration_bytes,
            registration_address,
            0x80,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "battle selected registration table",
        )?,
        validate_jump_target(
            &registration_bytes,
            registration_address,
            0x84,
            symbol_address(file, "ActRegList_Regist2")?,
            "battle selected-table registration call",
        )?,
        require_signed_immediate(
            &registration_bytes,
            registration_address,
            0x88,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            1,
            "battle selected registration file identifier",
        )?,
    ]);
    typed_registration_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkSequentialRegistrationValidation {
        predecessor_load_table_symbol: route.predecessor_load_table_symbol.clone(),
        predecessor_image_count,
        pointer_table_symbol: route.pointer_table_symbol.clone(),
        load_table_symbols: route.load_table_symbols.clone(),
        image_load_index: route.image_load_index,
        image_file_record: route.image_file_record,
        palette_slot: route.palette_slot,
        palette_file_record: route.palette_file_record,
        texture_slot: route.texture_slot,
        loader_actor_symbol: route.loader_actor_symbol.clone(),
        registration_actor_symbol: route.registration_actor_symbol.clone(),
        typed_loader_instructions,
        typed_registration_instructions,
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_indexed_registration_route(
    file: &object::File<'_>,
    executable: &[u8],
    route: &StylizedArtworkIndexedRegistrationRoute,
) -> Result<StylizedArtworkIndexedRegistrationValidation> {
    ensure!(
        !route.registration.load_table_symbol.trim().is_empty()
            && !route.registration.registration_set_symbol.trim().is_empty()
            && !route.load_actor_symbol.trim().is_empty()
            && route.load_actor_instruction_count > 0
            && !route.registration_actor_symbol.trim().is_empty()
            && route.registration_actor_instruction_count > 0,
        "indexed registration route is incomplete"
    );

    let (_, registration_bytes) =
        read_symbol_bytes(file, &route.registration.registration_set_symbol)?;
    let pairs = parse_texture_registration_pairs(
        &route.registration.registration_set_symbol,
        &registration_bytes,
    )?;
    ensure!(
        pairs
            .get(route.registration.texture_slot)
            .is_some_and(|pair| {
                pair.image_load_index == route.registration.image_load_index
                    && pair.palette_slot == route.registration.palette_slot
            }),
        "indexed registration pair changed"
    );

    ensure!(
        disassemble_symbol(executable, &route.load_actor_symbol)?
            .instructions
            .len()
            == route.load_actor_instruction_count,
        "indexed registration load actor instruction population changed"
    );
    let (load_actor_address, load_actor_bytes) = read_symbol_bytes(file, &route.load_actor_symbol)?;
    let mut typed_load_instructions = validate_loaded_address(
        &load_actor_bytes,
        load_actor_address,
        route.load_table_address_upper_offset,
        route.load_table_address_lower_offset,
        GeneralRegister::A1,
        symbol_address(file, &route.registration.load_table_symbol)?,
        "indexed registration load table",
    )?;
    typed_load_instructions.extend([
        require_signed_immediate(
            &load_actor_bytes,
            load_actor_address,
            route.load_file_id_instruction_offset,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "indexed registration load file identifier",
        )?,
        validate_jump_target(
            &load_actor_bytes,
            load_actor_address,
            route.load_call_offset,
            symbol_address(file, "aqCdvd_ReadFile")?,
            "indexed registration load call",
        )?,
    ]);
    typed_load_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    ensure!(
        disassemble_symbol(executable, &route.registration_actor_symbol)?
            .instructions
            .len()
            == route.registration_actor_instruction_count,
        "indexed registration actor instruction population changed"
    );
    let (registration_actor_address, registration_actor_bytes) =
        read_symbol_bytes(file, &route.registration_actor_symbol)?;
    let mut typed_registration_instructions = validate_loaded_address(
        &registration_actor_bytes,
        registration_actor_address,
        route.registration_load_table_address_upper_offset,
        route.registration_load_table_address_lower_offset,
        GeneralRegister::A1,
        symbol_address(file, &route.registration.load_table_symbol)?,
        "indexed registration actor load table",
    )?;
    typed_registration_instructions.extend(validate_loaded_address(
        &registration_actor_bytes,
        registration_actor_address,
        route.registration_set_address_upper_offset,
        route.registration_set_address_lower_offset,
        GeneralRegister::A2,
        symbol_address(file, &route.registration.registration_set_symbol)?,
        "indexed registration set",
    )?);
    typed_registration_instructions.extend([
        validate_jump_target(
            &registration_actor_bytes,
            registration_actor_address,
            route.registration_init_call_offset,
            symbol_address(file, "ActRegList_Init")?,
            "indexed registration initializer",
        )?,
        require_signed_immediate(
            &registration_actor_bytes,
            registration_actor_address,
            route.registration_file_id_instruction_offset,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "indexed registration file identifier",
        )?,
        validate_jump_target(
            &registration_actor_bytes,
            registration_actor_address,
            route.registration_call_offset,
            symbol_address(file, "ActRegList_Regist3")?,
            "indexed registration call",
        )?,
    ]);
    typed_registration_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkIndexedRegistrationValidation {
        registration: route.registration.clone(),
        file_id: route.file_id,
        load_actor_symbol: route.load_actor_symbol.clone(),
        registration_actor_symbol: route.registration_actor_symbol.clone(),
        typed_load_instructions,
        typed_registration_instructions,
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_demo_registration_route(
    file: &object::File<'_>,
    executable: &[u8],
    route: &StylizedArtworkDemoRegistrationRoute,
) -> Result<StylizedArtworkDemoRegistrationValidation> {
    const FIRST_GROUPED_DEMO_INDEX: usize = 0x4a;
    ensure!(
        !route.load_table_pointer_symbol.trim().is_empty()
            && !route.selected_load_table_symbol.trim().is_empty()
            && !route.registration_unit_pointer_symbol.trim().is_empty()
            && !route.registration_group_pointer_symbol.trim().is_empty()
            && !route.registration_set_symbol.trim().is_empty()
            && !route.transition_sequence_symbol.trim().is_empty()
            && route.demo_index >= FIRST_GROUPED_DEMO_INDEX
            && route.registration_unit_index == route.demo_index - FIRST_GROUPED_DEMO_INDEX
            && route.registration_pair_index == route.texture_slot
            && route.transition_opcode == 0x1f
            && route.transition_state == 2
            && route.transition_command_offsets.len() == route.registration_group_index
            && route.runner_symbol == "mDEMO_Run"
            && route.runner_instruction_count > 0
            && route.file_id == 2
            && route.registration_initializer_symbol == "ActRegList_Init"
            && route.registration_function_symbol == "ActRegList_Regist3",
        "demo texture registration route header changed"
    );
    ensure!(
        route
            .transition_command_offsets
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            == route.transition_command_offsets.len(),
        "demo transition command offsets are duplicated"
    );

    let load_table_pointers = read_pointer_table_addresses(file, &route.load_table_pointer_symbol)?;
    ensure!(
        load_table_pointers.get(route.demo_index).copied()
            == Some(symbol_address(file, &route.selected_load_table_symbol)?),
        "demo selected load-table pointer changed"
    );
    let registration_unit_pointers =
        read_pointer_table_addresses(file, &route.registration_unit_pointer_symbol)?;
    ensure!(
        registration_unit_pointers
            .get(route.registration_unit_index)
            .copied()
            == Some(symbol_address(
                file,
                &route.registration_group_pointer_symbol,
            )?),
        "demo registration-unit pointer changed"
    );
    let registration_group_pointers =
        read_pointer_table_addresses(file, &route.registration_group_pointer_symbol)?;
    ensure!(
        registration_group_pointers
            .get(route.registration_group_index)
            .copied()
            == Some(symbol_address(file, &route.registration_set_symbol)?),
        "demo registration-group pointer changed"
    );
    let (_, registration_bytes) = read_symbol_bytes(file, &route.registration_set_symbol)?;
    let registration_pairs = parse_image_sentinel_texture_registration_pairs(
        &route.registration_set_symbol,
        &registration_bytes,
    )?;
    ensure!(
        registration_pairs
            .get(route.registration_pair_index)
            .is_some_and(|pair| {
                pair.image_load_index == route.image_load_index
                    && pair.palette_slot == route.palette_slot
            }),
        "demo selected texture registration pair changed"
    );
    let selected_registration_pair = StylizedArtworkRegistrationRoute {
        load_table_symbol: route.selected_load_table_symbol.clone(),
        registration_set_symbol: route.registration_set_symbol.clone(),
        texture_slot: route.texture_slot,
        image_load_index: route.image_load_index,
        palette_slot: route.palette_slot,
    };

    let (_, command_size_bytes) = read_symbol_bytes(file, SEQUENCE_COMMAND_SIZE_SYMBOL)?;
    ensure!(
        command_size_bytes.len().is_multiple_of(2),
        "SeqComSize is not a sequence of 16-bit sizes"
    );
    let command_sizes = command_size_bytes
        .chunks_exact(2)
        .map(|bytes| usize::from(u16::from_le_bytes(bytes.try_into().expect("two-byte word"))))
        .collect::<Vec<_>>();
    ensure!(
        command_sizes
            .get(route.transition_opcode)
            .is_some_and(|words| *words == 2),
        "demo transition opcode size changed"
    );
    let (_, sequence_bytes) = read_symbol_bytes(file, &route.transition_sequence_symbol)?;
    for offset in &route.transition_command_offsets {
        let command = find_sequence_command(
            &route.transition_sequence_symbol,
            &sequence_bytes,
            &command_sizes,
            *offset,
        )?;
        ensure!(
            command.len() == 4
                && usize::from(u16::from_le_bytes(command[0..2].try_into()?))
                    == route.transition_opcode
                && u16::from_le_bytes(command[2..4].try_into()?) == route.transition_state,
            "demo transition sequence command changed"
        );
    }
    let typed_transition_decoder_instructions =
        validate_demo_transition_decoder(file, executable, route.transition_opcode)?;

    ensure!(
        disassemble_symbol(executable, &route.runner_symbol)?
            .instructions
            .len()
            == route.runner_instruction_count,
        "demo runner instruction population changed"
    );
    let (runner_address, runner_bytes) = read_symbol_bytes(file, &route.runner_symbol)?;
    let registration_unit_runtime_base =
        symbol_address(file, &route.registration_unit_pointer_symbol)?
            .checked_sub(u64::try_from(FIRST_GROUPED_DEMO_INDEX * 4)?)
            .context("demo registration-unit runtime base underflow")?;
    let mut typed_runner_instructions = validate_loaded_address(
        &runner_bytes,
        runner_address,
        0xa8,
        0xb0,
        GeneralRegister::V0,
        symbol_address(file, &route.load_table_pointer_symbol)?,
        "demo initial load-table pointer base",
    )?;
    typed_runner_instructions.extend(validate_loaded_address(
        &runner_bytes,
        runner_address,
        0xc0,
        0xc4,
        GeneralRegister::V0,
        registration_unit_runtime_base,
        "demo initial registration-unit pointer base",
    )?);
    typed_runner_instructions.extend(validate_loaded_address(
        &runner_bytes,
        runner_address,
        0x274,
        0x280,
        GeneralRegister::A1,
        symbol_address(file, &route.load_table_pointer_symbol)?,
        "demo transitioned load-table pointer base",
    )?);
    typed_runner_instructions.extend(validate_loaded_address(
        &runner_bytes,
        runner_address,
        0x27c,
        0x284,
        GeneralRegister::V1,
        registration_unit_runtime_base,
        "demo transitioned registration-unit pointer base",
    )?);
    typed_runner_instructions.extend([
        require_signed_immediate(
            &runner_bytes,
            runner_address,
            0xa0,
            InstructionForm::Slti,
            GeneralRegister::V0,
            GeneralRegister::V1,
            FIRST_GROUPED_DEMO_INDEX as i16,
            "demo grouped-route threshold",
        )?,
        validate_jump_target(
            &runner_bytes,
            runner_address,
            0x90,
            symbol_address(file, &route.registration_initializer_symbol)?,
            "demo initial registration initializer",
        )?,
        validate_jump_target(
            &runner_bytes,
            runner_address,
            0xe0,
            symbol_address(file, &route.registration_function_symbol)?,
            "demo initial texture registration",
        )?,
        require_signed_immediate(
            &runner_bytes,
            runner_address,
            0xe4,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "demo initial registration file identifier",
        )?,
        require_signed_immediate(
            &runner_bytes,
            runner_address,
            0x1f0,
            InstructionForm::Addiu,
            GeneralRegister::V0,
            GeneralRegister::ZERO,
            3,
            "demo special transition state",
        )?,
        require_signed_immediate(
            &runner_bytes,
            runner_address,
            0x1f8,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::A0,
            1,
            "demo registration group advance",
        )?,
        require_branch_two(
            &runner_bytes,
            runner_address,
            0x204,
            InstructionForm::Bne,
            GeneralRegister::V1,
            GeneralRegister::V0,
            runner_address + 0x268,
            "demo standard transition registration branch",
        )?,
        validate_jump_target(
            &runner_bytes,
            runner_address,
            0x268,
            symbol_address(file, &route.registration_initializer_symbol)?,
            "demo transitioned registration initializer",
        )?,
        validate_jump_target(
            &runner_bytes,
            runner_address,
            0x2b0,
            symbol_address(file, &route.registration_function_symbol)?,
            "demo transitioned texture registration",
        )?,
        require_signed_immediate(
            &runner_bytes,
            runner_address,
            0x2b4,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "demo transitioned registration file identifier",
        )?,
    ]);
    typed_runner_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkDemoRegistrationValidation {
        route: route.clone(),
        selected_registration_pair,
        typed_transition_decoder_instructions,
        typed_runner_instructions,
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_legacy_demo_registration_route(
    file: &object::File<'_>,
    executable: &[u8],
    route: &StylizedArtworkLegacyDemoRegistrationRoute,
) -> Result<StylizedArtworkLegacyDemoRegistrationValidation> {
    const FIRST_GROUPED_DEMO_INDEX: usize = 0x4a;
    ensure!(
        route.demo_index < FIRST_GROUPED_DEMO_INDEX
            && route.registration_pair_index == route.texture_slot
            && route.loader_symbol == "mDEMO_Init"
            && route.loader_instruction_count == 45
            && route.runner_symbol == "mDEMO_Run"
            && route.runner_instruction_count == 249
            && route.file_id == 2
            && route.read_function_symbol == "aqCdvd_ReadFile"
            && route.registration_initializer_symbol == "ActRegList_Init"
            && route.registration_function_symbol == "ActRegList_Regist3"
            && route.sequence_creator_symbol == "Seq_Create",
        "legacy demo registration route header changed"
    );

    let pointer_routes = [
        (
            route.load_table_pointer_symbol.as_str(),
            route.selected_load_table_symbol.as_str(),
        ),
        (
            route.pattern_table_pointer_symbol.as_str(),
            route.selected_pattern_table_symbol.as_str(),
        ),
        (
            route.animation_table_pointer_symbol.as_str(),
            route.selected_animation_table_symbol.as_str(),
        ),
        (
            route.movesys_table_pointer_symbol.as_str(),
            route.selected_movesys_table_symbol.as_str(),
        ),
        (
            route.sequence_table_pointer_symbol.as_str(),
            route.selected_sequence_symbol.as_str(),
        ),
    ];
    for (pointer_table_symbol, selected_symbol) in pointer_routes {
        ensure!(
            read_pointer_table_addresses(file, pointer_table_symbol)?
                .get(route.demo_index)
                .copied()
                == Some(symbol_address(file, selected_symbol)?),
            "legacy demo pointer selection changed: {pointer_table_symbol}"
        );
    }

    let (_, registration_bytes) = read_symbol_bytes(file, &route.registration_set_symbol)?;
    let registration_pairs =
        parse_texture_registration_pairs(&route.registration_set_symbol, &registration_bytes)?;
    ensure!(
        registration_pairs
            .get(route.registration_pair_index)
            .is_some_and(|pair| {
                pair.image_load_index == route.image_load_index
                    && pair.palette_slot == route.palette_slot
            }),
        "legacy demo selected texture registration pair changed"
    );
    let selected_registration_pair = StylizedArtworkRegistrationRoute {
        load_table_symbol: route.selected_load_table_symbol.clone(),
        registration_set_symbol: route.registration_set_symbol.clone(),
        texture_slot: route.texture_slot,
        image_load_index: route.image_load_index,
        palette_slot: route.palette_slot,
    };

    ensure!(
        disassemble_symbol(executable, &route.loader_symbol)?
            .instructions
            .len()
            == route.loader_instruction_count,
        "legacy demo loader instruction population changed"
    );
    let (loader_address, loader_bytes) = read_symbol_bytes(file, &route.loader_symbol)?;
    let mut typed_loader_instructions = validate_loaded_address(
        &loader_bytes,
        loader_address,
        0x04,
        0x14,
        GeneralRegister::V1,
        symbol_address(file, &route.sequence_table_pointer_symbol)?,
        "legacy demo sequence pointer-table address",
    )?;
    typed_loader_instructions.extend(validate_loaded_address(
        &loader_bytes,
        loader_address,
        0x50,
        0x58,
        GeneralRegister::V0,
        symbol_address(file, &route.load_table_pointer_symbol)?,
        "legacy demo load-table pointer address",
    )?);
    typed_loader_instructions.extend([
        require_shift(
            &loader_bytes,
            loader_address,
            0x18,
            InstructionForm::Sll,
            GeneralRegister::A1,
            GeneralRegister::A0,
            2,
            "legacy demo table index scale",
        )?,
        require_memory(
            &loader_bytes,
            loader_address,
            0x64,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "legacy demo selected load table",
        )?,
        validate_jump_target(
            &loader_bytes,
            loader_address,
            0x68,
            symbol_address(file, &route.read_function_symbol)?,
            "legacy demo texture read",
        )?,
        require_signed_immediate(
            &loader_bytes,
            loader_address,
            0x6c,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "legacy demo texture file identifier",
        )?,
    ]);
    typed_loader_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    ensure!(
        disassemble_symbol(executable, &route.runner_symbol)?
            .instructions
            .len()
            == route.runner_instruction_count,
        "legacy demo runner instruction population changed"
    );
    let (runner_address, runner_bytes) = read_symbol_bytes(file, &route.runner_symbol)?;
    let mut typed_runner_instructions = validate_loaded_address(
        &runner_bytes,
        runner_address,
        0xf8,
        0x100,
        GeneralRegister::V0,
        symbol_address(file, &route.load_table_pointer_symbol)?,
        "legacy demo registration load-table pointer address",
    )?;
    typed_runner_instructions.extend(validate_loaded_address(
        &runner_bytes,
        runner_address,
        0x108,
        0x118,
        GeneralRegister::A2,
        symbol_address(file, &route.registration_set_symbol)?,
        "legacy demo registration-set address",
    )?);
    for (upper_offset, lower_offset, register, symbol, role) in [
        (
            0x138,
            0x14c,
            GeneralRegister::A3,
            route.sequence_table_pointer_symbol.as_str(),
            "legacy demo sequence table address",
        ),
        (
            0x13c,
            0x150,
            GeneralRegister::A2,
            route.pattern_table_pointer_symbol.as_str(),
            "legacy demo pattern table address",
        ),
        (
            0x140,
            0x154,
            GeneralRegister::V1,
            route.animation_table_pointer_symbol.as_str(),
            "legacy demo animation table address",
        ),
        (
            0x144,
            0x158,
            GeneralRegister::V0,
            route.movesys_table_pointer_symbol.as_str(),
            "legacy demo movesys table address",
        ),
        (
            0x148,
            0x188,
            GeneralRegister::A1,
            SEQUENCE_COMMAND_SYMBOL,
            "legacy demo sequence decoder address",
        ),
    ] {
        typed_runner_instructions.extend(validate_loaded_address(
            &runner_bytes,
            runner_address,
            upper_offset,
            lower_offset,
            register,
            symbol_address(file, symbol)?,
            role,
        )?);
    }
    typed_runner_instructions.extend([
        validate_jump_target(
            &runner_bytes,
            runner_address,
            0x90,
            symbol_address(file, &route.registration_initializer_symbol)?,
            "legacy demo registration initializer",
        )?,
        require_signed_immediate(
            &runner_bytes,
            runner_address,
            0xa0,
            InstructionForm::Slti,
            GeneralRegister::V0,
            GeneralRegister::V1,
            FIRST_GROUPED_DEMO_INDEX as i16,
            "legacy demo route threshold",
        )?,
        require_signed_immediate(
            &runner_bytes,
            runner_address,
            0x110,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "legacy demo registration file identifier",
        )?,
        validate_jump_target(
            &runner_bytes,
            runner_address,
            0x114,
            symbol_address(file, &route.registration_function_symbol)?,
            "legacy demo texture registration",
        )?,
        validate_jump_target(
            &runner_bytes,
            runner_address,
            0x184,
            symbol_address(file, &route.sequence_creator_symbol)?,
            "legacy demo sequence creation",
        )?,
    ]);
    typed_runner_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkLegacyDemoRegistrationValidation {
        route: route.clone(),
        selected_registration_pair,
        typed_loader_instructions,
        typed_runner_instructions,
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_demo_transition_decoder(
    file: &object::File<'_>,
    executable: &[u8],
    transition_opcode: usize,
) -> Result<Vec<TypedInstructionEvidence>> {
    const DEMO_STATE_ADDRESS: u64 = 0x0150_cbb8;
    ensure!(
        (COMMAND_DISPATCH_FIRST_OPCODE..COMMAND_DISPATCH_FIRST_OPCODE + COMMAND_DISPATCH_ENTRIES)
            .contains(&transition_opcode),
        "demo transition opcode is outside the SeqCommand dispatch table"
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
        "demo transition command dispatch table",
    )?;
    let dispatch = read_address_range(
        file,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        u64::try_from(COMMAND_DISPATCH_ENTRIES * 4)?,
    )?;
    let entry_offset = (transition_opcode - COMMAND_DISPATCH_FIRST_OPCODE) * 4;
    let case_address = u64::from(u32::from_le_bytes(
        dispatch[entry_offset..entry_offset + 4]
            .try_into()
            .expect("fixed dispatch entry width"),
    ));
    let case_offset = usize::try_from(
        case_address
            .checked_sub(address)
            .context("demo transition case precedes SeqCommand")?,
    )?;
    evidence.push(require_memory(
        &source,
        address,
        case_offset,
        InstructionForm::Lhu,
        GeneralRegister::V1,
        GeneralRegister::V1,
        2,
        "demo transition state field",
    )?);
    evidence.extend(validate_loaded_memory_address(
        &source,
        address,
        case_offset + 0x04,
        case_offset + 0x14,
        GeneralRegister::AT,
        InstructionForm::Sh,
        GeneralRegister::V1,
        DEMO_STATE_ADDRESS,
        "demo transition state destination",
    )?);
    evidence.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(evidence)
}

pub(in crate::graphics_translation::stylized_artwork) fn read_pointer_table_addresses(
    file: &object::File<'_>,
    symbol: &str,
) -> Result<Vec<u64>> {
    let (_, bytes) = read_symbol_bytes(file, symbol)?;
    ensure!(
        bytes.len().is_multiple_of(4),
        "pointer table is not a sequence of four-byte addresses: {symbol}"
    );
    bytes
        .chunks_exact(4)
        .map(|chunk| Ok(u64::from(u32::from_le_bytes(chunk.try_into()?))))
        .collect()
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_pointer_table_symbols(
    file: &object::File<'_>,
    pointer_table_symbol: &str,
    expected_symbols: &[String],
) -> Result<()> {
    let addresses = read_pointer_table_addresses(file, pointer_table_symbol)?;
    ensure!(
        addresses.len() == expected_symbols.len(),
        "pointer table population changed: {pointer_table_symbol}"
    );
    for (address, symbol) in addresses.iter().copied().zip(expected_symbols) {
        ensure!(
            address == symbol_address(file, symbol)?,
            "pointer table entry changed: {pointer_table_symbol} -> {symbol}"
        );
    }
    Ok(())
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_face_state_action(
    file: &object::File<'_>,
    state_table_symbol: &str,
    state_entry_index: usize,
    action_symbol: &str,
) -> Result<()> {
    const STATE_RECORD_BYTES: usize = 12;
    let (_, bytes) = read_symbol_bytes(file, state_table_symbol)?;
    ensure!(
        bytes.len().is_multiple_of(STATE_RECORD_BYTES),
        "face state table is not a sequence of 12-byte records: {state_table_symbol}"
    );
    let mut action_addresses = Vec::new();
    let mut terminated = false;
    for record in bytes.chunks_exact(STATE_RECORD_BYTES) {
        let state = u16::from_le_bytes(record[0..2].try_into()?);
        if state == u16::MAX {
            ensure!(
                !terminated && record[2..].iter().all(|byte| *byte == 0),
                "face state terminator has nonzero fields: {state_table_symbol}"
            );
            terminated = true;
            continue;
        }
        ensure!(
            !terminated,
            "face state record follows the terminator: {state_table_symbol}"
        );
        action_addresses.push(u64::from(read_u32_at(record, 8)?));
    }
    ensure!(
        terminated
            && action_addresses.get(state_entry_index).copied()
                == Some(symbol_address(file, action_symbol)?),
        "face state action pointer changed: {state_table_symbol}"
    );
    Ok(())
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_face_action_pattern_indices(
    file: &object::File<'_>,
    action_symbol: &str,
    expected_pattern_indices: &[usize],
) -> Result<()> {
    const ACTION_RECORD_BYTES: usize = 4;
    let (_, bytes) = read_symbol_bytes(file, action_symbol)?;
    ensure!(
        bytes.len().is_multiple_of(ACTION_RECORD_BYTES),
        "face action is not a sequence of four-byte records: {action_symbol}"
    );
    let mut pattern_indices = Vec::new();
    let mut terminated = false;
    for record in bytes.chunks_exact(ACTION_RECORD_BYTES) {
        let pattern_index = u16::from_le_bytes(record[0..2].try_into()?);
        let duration = u16::from_le_bytes(record[2..4].try_into()?);
        if pattern_index == u16::MAX || pattern_index == u16::MAX - 1 {
            ensure!(
                !terminated && duration == 0,
                "face action terminator has a nonzero duration: {action_symbol}"
            );
            terminated = true;
            continue;
        }
        ensure!(
            !terminated && duration > 0,
            "face action record is malformed: {action_symbol}"
        );
        pattern_indices.push(usize::from(pattern_index));
    }
    ensure!(
        terminated && pattern_indices == expected_pattern_indices,
        "face action pattern sequence changed: {action_symbol}"
    );
    Ok(())
}

pub(in crate::graphics_translation::stylized_artwork) fn load_table_image_count(
    file: &object::File<'_>,
    symbol: &str,
) -> Result<usize> {
    let (_, bytes) = read_symbol_bytes(file, symbol)?;
    Ok(parse_load_records(&bytes)?
        .iter()
        .filter(|record| record.operation == 1)
        .count())
}

pub(in crate::graphics_translation::stylized_artwork) fn uniform_load_table_image_count(
    file: &object::File<'_>,
    symbols: &[String],
    role: &str,
) -> Result<usize> {
    let counts = symbols
        .iter()
        .map(|symbol| load_table_image_count(file, symbol))
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        counts.len() == 1,
        "{role} load tables no longer register a uniform image count"
    );
    counts
        .into_iter()
        .next()
        .context("character-face registration route has no load tables")
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_character_face_registration_route(
    file: &object::File<'_>,
    executable: &[u8],
    route: &StylizedArtworkCharacterFaceRegistrationRoute,
) -> Result<StylizedArtworkCharacterFaceRegistrationValidation> {
    ensure!(
        route.load_actor_symbol == "Puyo_2P_Init"
            && route.load_actor_instruction_count == 94
            && route.registration_actor_symbol == "Puyo_2P_Run"
            && route.registration_actor_instruction_count == 588
            && !route.face_load_table_pointer_symbol.trim().is_empty()
            && !route.selected_face_load_table_symbol.trim().is_empty()
            && !route.frame_load_table_symbols.is_empty()
            && !route.game_load_table_symbols.is_empty()
            && !route.registration_set_symbol.trim().is_empty()
            && route.random_character_count > 0
            && route.random_character_offset >= 0,
        "character-face registration route header changed"
    );
    let random_character_start = usize::try_from(route.random_character_offset)?;
    let random_character_end = random_character_start
        .checked_add(usize::try_from(route.random_character_count)?)
        .context("character-face random character range overflow")?;
    ensure!(
        route.character_index >= random_character_start
            && route.character_index < random_character_end
            && route.character_index < route.character_count,
        "selected character is not reachable through the declared random range"
    );
    ensure!(
        route
            .frame_load_table_symbols
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            .len()
            == route.frame_load_table_symbols.len()
            && route
                .game_load_table_symbols
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                == route.game_load_table_symbols.len(),
        "character-face registration route repeats a load table"
    );

    validate_pointer_table_symbols(
        file,
        &route.frame_load_table_pointer_symbol,
        &route.frame_load_table_symbols,
    )?;
    validate_pointer_table_symbols(
        file,
        &route.game_load_table_pointer_symbol,
        &route.game_load_table_symbols,
    )?;
    let face_addresses = read_pointer_table_addresses(file, &route.face_load_table_pointer_symbol)?;
    ensure!(
        face_addresses.len() == route.character_count
            && face_addresses.get(route.character_index).copied()
                == Some(symbol_address(
                    file,
                    &route.selected_face_load_table_symbol
                )?),
        "selected character face load-table pointer changed"
    );

    let frame_image_count =
        uniform_load_table_image_count(file, &route.frame_load_table_symbols, "character frame")?;
    let game_image_count =
        uniform_load_table_image_count(file, &route.game_load_table_symbols, "game")?;
    let derived_texture_slot = frame_image_count
        .checked_add(game_image_count)
        .and_then(|count| count.checked_add(route.registration_pair_index))
        .context("character-face texture slot overflow")?;
    ensure!(
        route.texture_slot == derived_texture_slot,
        "character-face texture slot no longer follows frame and game registrations"
    );

    if let Some(fixed) = &route.fixed_face_pattern {
        let (_, face_load_bytes) = read_symbol_bytes(file, &route.selected_face_load_table_symbol)?;
        let face_loads = parse_load_records(&face_load_bytes)?;
        let images = face_loads
            .iter()
            .filter(|record| record.operation == 1)
            .collect::<Vec<_>>();
        let palettes = face_loads
            .iter()
            .filter(|record| record.operation == 2)
            .collect::<Vec<_>>();
        ensure!(
            images.get(fixed.image_load_index).is_some_and(|record| {
                record.file_record == fixed.image_file_record
                    && record.flags == fixed.image_flags
                    && record.destination == fixed.image_destination
            }) && palettes.get(fixed.palette_slot).is_some_and(|record| {
                record.file_record == fixed.palette_file_record
                    && record.flags == fixed.palette_flags
                    && record.destination == fixed.palette_destination
            }),
            "fixed character-face image or palette load record changed"
        );
        ensure!(
            fixed.texture_slot
                == derived_texture_slot
                    .checked_add(1)
                    .context("fixed character-face texture slot overflow")?,
            "fixed character-face texture slot no longer follows assembled registrations"
        );
    }

    let (_, registration_bytes) = read_symbol_bytes(file, &route.registration_set_symbol)?;
    let registration_pairs =
        parse_texture_registration_pairs(&route.registration_set_symbol, &registration_bytes)?;
    ensure!(
        registration_pairs.len() == route.registration_pair_index + 1
            && registration_pairs
                .get(route.registration_pair_index)
                .is_some_and(|pair| {
                    pair.image_load_index == route.image_load_index
                        && pair.palette_slot == route.registration_palette_slot
                }),
        "character-face texture registration pair changed"
    );

    ensure!(
        disassemble_symbol(executable, &route.load_actor_symbol)?
            .instructions
            .len()
            == route.load_actor_instruction_count,
        "character-face load actor instruction population changed"
    );
    let (load_address, load_bytes) = read_symbol_bytes(file, &route.load_actor_symbol)?;
    let mut typed_load_instructions = validate_loaded_address(
        &load_bytes,
        load_address,
        0x20,
        0x24,
        GeneralRegister::V0,
        symbol_address(file, &route.frame_load_table_pointer_symbol)?,
        "character frame load-table pointer table",
    )?;
    typed_load_instructions.extend(validate_loaded_address(
        &load_bytes,
        load_address,
        0x50,
        0x58,
        GeneralRegister::V0,
        symbol_address(file, &route.game_load_table_pointer_symbol)?,
        "game load-table pointer table",
    )?);
    typed_load_instructions.extend(validate_loaded_address(
        &load_bytes,
        load_address,
        0x74,
        0x7c,
        GeneralRegister::V0,
        symbol_address(file, &route.face_load_table_pointer_symbol)?,
        "character face load-table pointer table",
    )?);
    typed_load_instructions.extend([
        validate_jump_target(
            &load_bytes,
            load_address,
            0x08,
            symbol_address(file, "GetRand")?,
            "character selector random call",
        )?,
        require_signed_immediate(
            &load_bytes,
            load_address,
            0x0c,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.random_character_count,
            "character selector random population",
        )?,
        require_signed_immediate(
            &load_bytes,
            load_address,
            0x1c,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::V0,
            route.random_character_offset,
            "character selector index offset",
        )?,
        require_memory(
            &load_bytes,
            load_address,
            0x30,
            InstructionForm::Sh,
            GeneralRegister::A0,
            GeneralRegister::AT,
            -13_510,
            "selected character store",
        )?,
        require_memory(
            &load_bytes,
            load_address,
            0x18,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_432,
            "character frame selector",
        )?,
        require_shift(
            &load_bytes,
            load_address,
            0x2c,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "character frame selector scale",
        )?,
        require_three_general(
            &load_bytes,
            load_address,
            0x34,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "selected character frame pointer",
        )?,
        require_memory(
            &load_bytes,
            load_address,
            0x38,
            InstructionForm::Lw,
            GeneralRegister::V0,
            GeneralRegister::V0,
            0,
            "selected character frame load table",
        )?,
        require_memory(
            &load_bytes,
            load_address,
            0x3c,
            InstructionForm::Sw,
            GeneralRegister::V0,
            GeneralRegister::GP,
            -29_736,
            "selected character frame table store",
        )?,
        require_memory(
            &load_bytes,
            load_address,
            0x40,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::GP,
            -29_736,
            "selected character frame table reload",
        )?,
        validate_jump_target(
            &load_bytes,
            load_address,
            0x44,
            symbol_address(file, "aqCdvd_ReadFile")?,
            "selected character frame load call",
        )?,
        require_signed_immediate(
            &load_bytes,
            load_address,
            0x48,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "selected character frame file identifier",
        )?,
        require_memory(
            &load_bytes,
            load_address,
            0x54,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_380,
            "game load-table selector",
        )?,
        require_shift(
            &load_bytes,
            load_address,
            0x5c,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "game load-table selector scale",
        )?,
        require_three_general(
            &load_bytes,
            load_address,
            0x60,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "selected game load-table pointer",
        )?,
        require_memory(
            &load_bytes,
            load_address,
            0x64,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "selected game load table",
        )?,
        validate_jump_target(
            &load_bytes,
            load_address,
            0x68,
            symbol_address(file, "aqCdvd_ReadFile")?,
            "selected game load call",
        )?,
        require_signed_immediate(
            &load_bytes,
            load_address,
            0x6c,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "selected game file identifier",
        )?,
        require_memory(
            &load_bytes,
            load_address,
            0x78,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_510,
            "character face selector",
        )?,
        require_shift(
            &load_bytes,
            load_address,
            0x80,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "character face selector scale",
        )?,
        require_three_general(
            &load_bytes,
            load_address,
            0x84,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "selected character face pointer",
        )?,
        require_memory(
            &load_bytes,
            load_address,
            0x88,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "selected character face load table",
        )?,
        validate_jump_target(
            &load_bytes,
            load_address,
            0x8c,
            symbol_address(file, "aqCdvd_ReadFile")?,
            "selected character face load call",
        )?,
        require_signed_immediate(
            &load_bytes,
            load_address,
            0x90,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "selected character face file identifier",
        )?,
    ]);
    typed_load_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    ensure!(
        disassemble_symbol(executable, &route.registration_actor_symbol)?
            .instructions
            .len()
            == route.registration_actor_instruction_count,
        "character-face registration actor instruction population changed"
    );
    let (registration_address, registration_actor_bytes) =
        read_symbol_bytes(file, &route.registration_actor_symbol)?;
    let mut typed_registration_instructions = validate_loaded_address(
        &registration_actor_bytes,
        registration_address,
        0xe8,
        0xf0,
        GeneralRegister::V0,
        symbol_address(file, &route.game_load_table_pointer_symbol)?,
        "game registration pointer table",
    )?;
    typed_registration_instructions.extend(validate_loaded_address(
        &registration_actor_bytes,
        registration_address,
        0x168,
        0x174,
        GeneralRegister::V0,
        symbol_address(file, &route.face_load_table_pointer_symbol)?,
        "character face registration pointer table",
    )?);
    typed_registration_instructions.extend(validate_loaded_address(
        &registration_actor_bytes,
        registration_address,
        0x170,
        0x18c,
        GeneralRegister::A2,
        symbol_address(file, &route.registration_set_symbol)?,
        "character face registration set",
    )?);
    typed_registration_instructions.extend([
        validate_jump_target(
            &registration_actor_bytes,
            registration_address,
            0xd0,
            symbol_address(file, "ActRegList_Init")?,
            "character-face registration initializer",
        )?,
        require_memory(
            &registration_actor_bytes,
            registration_address,
            0xd8,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::GP,
            -29_736,
            "selected character frame registration table",
        )?,
        validate_jump_target(
            &registration_actor_bytes,
            registration_address,
            0xdc,
            symbol_address(file, "ActRegList_Regist2")?,
            "selected character frame registration call",
        )?,
        require_signed_immediate(
            &registration_actor_bytes,
            registration_address,
            0xe0,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "selected character frame registration file identifier",
        )?,
        require_memory(
            &registration_actor_bytes,
            registration_address,
            0xec,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_380,
            "game registration selector",
        )?,
        require_shift(
            &registration_actor_bytes,
            registration_address,
            0xf4,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "game registration selector scale",
        )?,
        require_three_general(
            &registration_actor_bytes,
            registration_address,
            0xf8,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "selected game registration pointer",
        )?,
        require_memory(
            &registration_actor_bytes,
            registration_address,
            0xfc,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "selected game registration table",
        )?,
        validate_jump_target(
            &registration_actor_bytes,
            registration_address,
            0x100,
            symbol_address(file, "ActRegList_Regist2")?,
            "selected game registration call",
        )?,
        require_signed_immediate(
            &registration_actor_bytes,
            registration_address,
            0x104,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "selected game registration file identifier",
        )?,
        require_memory(
            &registration_actor_bytes,
            registration_address,
            0x16c,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_510,
            "character face registration selector",
        )?,
        require_signed_immediate(
            &registration_actor_bytes,
            registration_address,
            0x178,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            route.file_id,
            "character face registration file identifier",
        )?,
        require_shift(
            &registration_actor_bytes,
            registration_address,
            0x17c,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "character face registration selector scale",
        )?,
        require_three_general(
            &registration_actor_bytes,
            registration_address,
            0x180,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "selected character face registration pointer",
        )?,
        require_memory(
            &registration_actor_bytes,
            registration_address,
            0x184,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "selected character face registration table",
        )?,
        validate_jump_target(
            &registration_actor_bytes,
            registration_address,
            0x188,
            symbol_address(file, "ActRegList_Regist3")?,
            "selected character face registration call",
        )?,
        validate_jump_target(
            &registration_actor_bytes,
            registration_address,
            0x190,
            symbol_address(file, "ActRegList_FacePtn")?,
            "assembled character face registration call",
        )?,
    ]);
    if let Some(fixed) = &route.fixed_face_pattern {
        ensure!(
            fixed.registration_function_symbol == "ActRegList_FacePtn"
                && fixed.registration_function_instruction_count > 0
                && fixed.registration_width_field > 0
                && fixed.registration_height_field > 0
                && fixed.pixel_storage_format >= 0,
            "fixed character-face registration declaration changed"
        );
        ensure!(
            disassemble_symbol(executable, &fixed.registration_function_symbol)?
                .instructions
                .len()
                == fixed.registration_function_instruction_count,
            "fixed character-face registration instruction population changed"
        );
        let (fixed_address, fixed_bytes) =
            read_symbol_bytes(file, &fixed.registration_function_symbol)?;
        typed_registration_instructions.extend([
            require_signed_immediate(
                &fixed_bytes,
                fixed_address,
                0x40,
                InstructionForm::Addiu,
                GeneralRegister::A3,
                GeneralRegister::ZERO,
                i16::try_from(fixed.palette_destination)?,
                "fixed character-face palette destination",
            )?,
            require_signed_immediate(
                &fixed_bytes,
                fixed_address,
                0x44,
                InstructionForm::Addiu,
                GeneralRegister::A2,
                GeneralRegister::ZERO,
                fixed.registration_width_field,
                "fixed character-face registration width field",
            )?,
            require_signed_immediate(
                &fixed_bytes,
                fixed_address,
                0x48,
                InstructionForm::Addiu,
                GeneralRegister::A1,
                GeneralRegister::ZERO,
                fixed.registration_height_field,
                "fixed character-face registration height field",
            )?,
            require_signed_immediate(
                &fixed_bytes,
                fixed_address,
                0x50,
                InstructionForm::Addiu,
                GeneralRegister::V1,
                GeneralRegister::ZERO,
                fixed.pixel_storage_format,
                "fixed character-face pixel storage format",
            )?,
        ]);
    }
    typed_registration_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkCharacterFaceRegistrationValidation {
        route: route.clone(),
        frame_image_count,
        game_image_count,
        typed_load_instructions,
        typed_registration_instructions,
    })
}
