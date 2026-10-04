use super::*;

pub(super) struct BattlePatternActionEvidence {
    pub(super) pattern_source_bounds: PixelBounds,
    pub(super) typed_actor_instructions: Vec<TypedInstructionEvidence>,
    pub(super) typed_creator_instructions: Vec<TypedInstructionEvidence>,
}

pub(super) fn validate_character_action_pattern_record(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkBattleTextureConsumer,
    character_face_index: Option<usize>,
    texture_use: &StylizedArtworkBattleTextureUse,
) -> Result<BattlePatternActionEvidence> {
    let StylizedArtworkBattleTextureUse::CharacterActionPatternRecord {
        pattern_pointer_table_symbol,
        character_index,
        pattern_table_symbol,
        pattern_record_index,
        pattern_source_bounds,
        pattern_pointer_address_upper_offset,
        pattern_pointer_address_lower_offset,
        character_selector_offset,
        character_selector_scale_offset,
        selected_pattern_table_offset,
        pattern_initializer_call_offset,
        action_pointer_table_symbol,
        action_table_symbol,
        action_pattern_indices,
        action_pointer_address_upper_offset,
        action_pointer_address_lower_offset,
        action_character_selector_offset,
        action_character_selector_scale_offset,
        selected_action_table_offset,
    } = texture_use
    else {
        unreachable!("character action validator received another texture use")
    };
    ensure!(
        character_face_index == Some(*character_index),
        "character action pattern disagrees with its face registration route"
    );
    validate_selected_pattern(
        file,
        consumer.pattern_texture_slot,
        pattern_pointer_table_symbol,
        *character_index,
        pattern_table_symbol,
        *pattern_record_index,
        *pattern_source_bounds,
    )?;
    let action_addresses = read_pointer_table_addresses(file, action_pointer_table_symbol)?;
    ensure!(
        action_addresses.get(*character_index).copied()
            == Some(symbol_address(file, action_table_symbol)?),
        "character action pointer-table entry changed"
    );
    validate_face_action_pattern_indices(file, action_table_symbol, action_pattern_indices)?;
    ensure!(
        action_pattern_indices.contains(pattern_record_index),
        "character action no longer selects the declared pattern record"
    );

    ensure!(
        disassemble_symbol(executable, &consumer.actor_symbol)?
            .instructions
            .len()
            == consumer.actor_instruction_count
            && consumer.pattern_setter_symbol == "Act_AnimeInit"
            && consumer.pattern_setter_call_offset
                == selected_action_table_offset
                    .checked_add(8)
                    .context("character action call offset overflow")?,
        "character action actor shape or animation initializer changed"
    );
    let (actor_address, actor_bytes) = read_symbol_bytes(file, &consumer.actor_symbol)?;
    let mut typed_actor_instructions = validate_loaded_address(
        &actor_bytes,
        actor_address,
        *pattern_pointer_address_upper_offset,
        *pattern_pointer_address_lower_offset,
        GeneralRegister::V0,
        symbol_address(file, pattern_pointer_table_symbol)?,
        "character action pattern pointer table",
    )?;
    typed_actor_instructions.extend([
        require_memory(
            &actor_bytes,
            actor_address,
            *character_selector_offset,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_510,
            "character action pattern selector",
        )?,
        require_shift(
            &actor_bytes,
            actor_address,
            *character_selector_scale_offset,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "character action pattern selector scale",
        )?,
        require_three_general(
            &actor_bytes,
            actor_address,
            *selected_pattern_table_offset,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "selected character pattern table",
        )?,
        validate_jump_target(
            &actor_bytes,
            actor_address,
            *pattern_initializer_call_offset,
            symbol_address(file, "Act_CellInit")?,
            "character action pattern initializer",
        )?,
        require_memory(
            &actor_bytes,
            actor_address,
            pattern_initializer_call_offset
                .checked_add(4)
                .context("character pattern pointer load offset overflow")?,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "selected character pattern-table pointer",
        )?,
    ]);
    typed_actor_instructions.extend(validate_loaded_address(
        &actor_bytes,
        actor_address,
        *action_pointer_address_upper_offset,
        *action_pointer_address_lower_offset,
        GeneralRegister::V0,
        symbol_address(file, action_pointer_table_symbol)?,
        "character action pointer table",
    )?);
    typed_actor_instructions.extend([
        require_memory(
            &actor_bytes,
            actor_address,
            *action_character_selector_offset,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_510,
            "character action selector",
        )?,
        require_shift(
            &actor_bytes,
            actor_address,
            *action_character_selector_scale_offset,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "character action selector scale",
        )?,
        require_three_general(
            &actor_bytes,
            actor_address,
            *selected_action_table_offset,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "selected character action",
        )?,
        require_memory(
            &actor_bytes,
            actor_address,
            selected_action_table_offset
                .checked_add(4)
                .context("character action pointer load offset overflow")?,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "selected character action pointer",
        )?,
    ]);
    Ok(BattlePatternActionEvidence {
        pattern_source_bounds: *pattern_source_bounds,
        typed_actor_instructions,
        typed_creator_instructions: Vec::new(),
    })
}

pub(super) fn validate_face_condition_pattern_record(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkBattleTextureConsumer,
    character_face_index: Option<usize>,
    texture_use: &StylizedArtworkBattleTextureUse,
) -> Result<BattlePatternActionEvidence> {
    let StylizedArtworkBattleTextureUse::FaceConditionPatternRecord {
        pattern_pointer_table_symbol,
        character_index,
        pattern_table_symbol,
        pattern_record_index,
        pattern_source_bounds,
        creator_actor_symbol,
        creator_actor_instruction_count,
        creator_function_address_upper_offset,
        creator_function_address_lower_offset,
        creator_call_offset,
        pattern_pointer_address_upper_offset,
        pattern_pointer_address_lower_offset,
        character_selector_offset,
        character_selector_scale_offset,
        selected_pattern_table_offset,
        pattern_initializer_call_offset,
        initial_pattern_setter_call_offset,
        actor_pattern_pointer_load_offset,
        actor_pattern_pointer_adjustment_offset,
        actor_pattern_byte_offset,
    } = texture_use
    else {
        unreachable!("face condition validator received another texture use")
    };
    ensure!(
        character_face_index == Some(*character_index),
        "face condition pattern disagrees with its face registration route"
    );
    validate_selected_pattern(
        file,
        consumer.pattern_texture_slot,
        pattern_pointer_table_symbol,
        *character_index,
        pattern_table_symbol,
        *pattern_record_index,
        *pattern_source_bounds,
    )?;
    ensure!(
        usize::try_from(*actor_pattern_byte_offset)?
            == pattern_record_index
                .checked_mul(24)
                .context("face condition pattern offset overflow")?
            && consumer.pattern_setter_symbol == "Act_SetPtn",
        "face condition actor pattern offset changed"
    );

    let (actor_address, actor_bytes) = read_symbol_bytes(file, &consumer.actor_symbol)?;
    let mut typed_actor_instructions = Vec::new();
    if *actor_pattern_byte_offset == 0 {
        ensure!(
            actor_pattern_pointer_adjustment_offset.is_none(),
            "zero face condition pattern unexpectedly declares an adjustment"
        );
        typed_actor_instructions.push(require_memory(
            &actor_bytes,
            actor_address,
            *actor_pattern_pointer_load_offset,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::S0,
            64,
            "face condition base pattern pointer",
        )?);
    } else {
        let adjustment_offset = actor_pattern_pointer_adjustment_offset
            .context("nonzero face condition pattern has no pointer adjustment")?;
        typed_actor_instructions.extend([
            require_memory(
                &actor_bytes,
                actor_address,
                *actor_pattern_pointer_load_offset,
                InstructionForm::Lw,
                GeneralRegister::V0,
                GeneralRegister::S0,
                64,
                "face condition base pattern pointer",
            )?,
            require_signed_immediate(
                &actor_bytes,
                actor_address,
                adjustment_offset,
                InstructionForm::Addiu,
                GeneralRegister::A1,
                GeneralRegister::V0,
                *actor_pattern_byte_offset,
                "face condition selected pattern pointer",
            )?,
        ]);
    }

    ensure!(
        disassemble_symbol(executable, creator_actor_symbol)?
            .instructions
            .len()
            == *creator_actor_instruction_count,
        "face condition creator instruction population changed"
    );
    let (creator_address, creator_bytes) = read_symbol_bytes(file, creator_actor_symbol)?;
    let mut typed_creator_instructions = validate_loaded_address(
        &creator_bytes,
        creator_address,
        *creator_function_address_upper_offset,
        *creator_function_address_lower_offset,
        GeneralRegister::A0,
        symbol_address(file, &consumer.actor_symbol)?,
        "face condition actor function",
    )?;
    typed_creator_instructions.push(validate_jump_target(
        &creator_bytes,
        creator_address,
        *creator_call_offset,
        symbol_address(file, "Act_SetBottom")?,
        "face condition actor creation call",
    )?);
    typed_creator_instructions.extend(validate_loaded_address(
        &creator_bytes,
        creator_address,
        *pattern_pointer_address_upper_offset,
        *pattern_pointer_address_lower_offset,
        GeneralRegister::V0,
        symbol_address(file, pattern_pointer_table_symbol)?,
        "face condition pattern pointer table",
    )?);
    typed_creator_instructions.extend([
        require_memory(
            &creator_bytes,
            creator_address,
            *character_selector_offset,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::AT,
            -13_510,
            "face condition character selector",
        )?,
        require_shift(
            &creator_bytes,
            creator_address,
            *character_selector_scale_offset,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "face condition character selector scale",
        )?,
        require_three_general(
            &creator_bytes,
            creator_address,
            *selected_pattern_table_offset,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "selected face condition pattern table",
        )?,
        require_memory(
            &creator_bytes,
            creator_address,
            pattern_initializer_call_offset
                .checked_sub(4)
                .context("face condition pattern pointer load offset underflow")?,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "selected face condition pattern-table pointer",
        )?,
        validate_jump_target(
            &creator_bytes,
            creator_address,
            *pattern_initializer_call_offset,
            symbol_address(file, "Act_CellInit")?,
            "face condition cell initializer",
        )?,
        require_memory(
            &creator_bytes,
            creator_address,
            initial_pattern_setter_call_offset
                .checked_sub(4)
                .context("face condition initial pattern load offset underflow")?,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::S0,
            64,
            "face condition initial pattern pointer",
        )?,
        validate_jump_target(
            &creator_bytes,
            creator_address,
            *initial_pattern_setter_call_offset,
            symbol_address(file, "Act_SetPtn")?,
            "face condition initial pattern setter",
        )?,
    ]);
    Ok(BattlePatternActionEvidence {
        pattern_source_bounds: *pattern_source_bounds,
        typed_actor_instructions,
        typed_creator_instructions,
    })
}

fn validate_selected_pattern(
    file: &object::File<'_>,
    texture_slot: usize,
    pattern_pointer_table_symbol: &str,
    character_index: usize,
    pattern_table_symbol: &str,
    pattern_record_index: usize,
    pattern_source_bounds: PixelBounds,
) -> Result<()> {
    let pointer_addresses = read_pointer_table_addresses(file, pattern_pointer_table_symbol)?;
    ensure!(
        pointer_addresses.get(character_index).copied()
            == Some(symbol_address(file, pattern_table_symbol)?),
        "character pattern-table pointer changed"
    );
    let (_, pattern_bytes) = read_symbol_bytes(file, pattern_table_symbol)?;
    let patterns = parse_pattern_records(pattern_table_symbol, &pattern_bytes)?;
    let pattern = patterns
        .get(pattern_record_index)
        .context("character action pattern record is out of range")?;
    ensure!(
        pattern.image_load_index == texture_slot
            && pattern.source_x == pattern_source_bounds.x
            && pattern.source_y == pattern_source_bounds.y
            && pattern.width == pattern_source_bounds.width
            && pattern.height == pattern_source_bounds.height,
        "character action pattern record changed"
    );
    Ok(())
}
