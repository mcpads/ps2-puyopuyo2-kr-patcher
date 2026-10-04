use super::*;

#[path = "battle/pattern_actions.rs"]
mod pattern_actions;
use pattern_actions::{
    validate_character_action_pattern_record, validate_face_condition_pattern_record,
};

pub(in crate::graphics_translation::stylized_artwork) fn validate_battle_texture_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkBattleTextureConsumer,
    region: &super::GraphicsTextRegion,
    registration_route: StylizedArtworkBattleRegistrationRouteRef<'_>,
) -> Result<StylizedArtworkBattleTextureConsumerValidation> {
    let character_face_index = match registration_route {
        StylizedArtworkBattleRegistrationRouteRef::Sequential(route) => {
            ensure!(
                route
                    .load_table_symbols
                    .contains(&consumer.load_route.load_table_symbol)
                    && consumer.load_route.image_load_index == route.image_load_index
                    && consumer.load_route.image_file_record == route.image_file_record
                    && consumer.load_route.palette_slot == route.palette_slot
                    && consumer.load_route.palette_file_record == route.palette_file_record
                    && consumer.pattern_texture_slot == route.texture_slot,
                "battle texture consumer disagrees with its sequential registration route"
            );
            None
        }
        StylizedArtworkBattleRegistrationRouteRef::Indexed(route) => {
            let registration = &route.registration;
            ensure!(
                consumer.load_route.load_table_symbol == registration.load_table_symbol
                    && consumer.load_route.image_load_index == registration.image_load_index
                    && consumer.load_route.palette_slot == registration.palette_slot
                    && consumer.pattern_texture_slot == registration.texture_slot,
                "battle texture consumer disagrees with its indexed registration route"
            );
            let (_, load_bytes) = read_symbol_bytes(file, &consumer.load_route.load_table_symbol)?;
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
                    .get(consumer.load_route.image_load_index)
                    .is_some_and(|record| {
                        record.file_record == consumer.load_route.image_file_record
                    })
                    && palettes
                        .get(consumer.load_route.palette_slot)
                        .is_some_and(|record| {
                            record.file_record == consumer.load_route.palette_file_record
                        }),
                "battle indexed load-table route changed"
            );
            None
        }
        StylizedArtworkBattleRegistrationRouteRef::CharacterFace(route) => {
            let matches_registered_pair = consumer.load_route.image_load_index
                == route.image_load_index
                && consumer.load_route.palette_slot == route.palette_load_index
                && consumer.pattern_texture_slot == route.texture_slot;
            let matches_fixed_pattern = route.fixed_face_pattern.as_ref().is_some_and(|fixed| {
                consumer.load_route.image_load_index == fixed.image_load_index
                    && consumer.load_route.image_file_record == fixed.image_file_record
                    && consumer.load_route.palette_slot == fixed.palette_slot
                    && consumer.load_route.palette_file_record == fixed.palette_file_record
                    && consumer.pattern_texture_slot == fixed.texture_slot
            });
            ensure!(
                consumer.load_route.load_table_symbol == route.selected_face_load_table_symbol
                    && (matches_registered_pair || matches_fixed_pattern),
                "battle texture consumer disagrees with its character-face registration route"
            );
            let (_, load_bytes) = read_symbol_bytes(file, &consumer.load_route.load_table_symbol)?;
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
                    .get(consumer.load_route.image_load_index)
                    .is_some_and(|record| {
                        record.file_record == consumer.load_route.image_file_record
                    })
                    && palettes
                        .get(consumer.load_route.palette_slot)
                        .is_some_and(|record| {
                            record.file_record == consumer.load_route.palette_file_record
                        }),
                "battle character-face load-table route changed"
            );
            Some(route.character_index)
        }
    };
    ensure!(
        disassemble_symbol(executable, &consumer.actor_symbol)?
            .instructions
            .len()
            == consumer.actor_instruction_count,
        "battle texture actor instruction population changed"
    );
    let (actor_address, actor_bytes) = read_symbol_bytes(file, &consumer.actor_symbol)?;
    let mut typed_actor_instructions = vec![validate_jump_target(
        &actor_bytes,
        actor_address,
        consumer.pattern_setter_call_offset,
        symbol_address(file, &consumer.pattern_setter_symbol)?,
        "battle texture setter call",
    )?];
    let mut typed_creator_instructions = Vec::new();

    let pattern_source_bounds = match &consumer.texture_use {
        StylizedArtworkBattleTextureUse::PatternRecord {
            pattern_table_symbol,
            pattern_address_record_index,
            pattern_record_index,
            pattern_iteration_index,
            pattern_iteration_count,
            pattern_source_bounds,
            pattern_address_upper_offset,
            pattern_address_lower_offset,
            pattern_argument_offset,
            pattern_stride_instruction_offset,
            pattern_iteration_count_instruction_offset,
        } => {
            let (_, pattern_bytes) = read_symbol_bytes(file, pattern_table_symbol)?;
            let patterns = parse_pattern_records(pattern_table_symbol, &pattern_bytes)?;
            let pattern = patterns
                .get(*pattern_record_index)
                .context("battle texture pattern record is out of range")?;
            ensure!(
                pattern.image_load_index == consumer.pattern_texture_slot
                    && pattern.source_x == pattern_source_bounds.x
                    && pattern.source_y == pattern_source_bounds.y
                    && pattern.width == pattern_source_bounds.width
                    && pattern.height == pattern_source_bounds.height,
                "battle texture pattern record changed"
            );
            ensure!(
                *pattern_record_index == *pattern_address_record_index + *pattern_iteration_index
                    && *pattern_iteration_count > 0
                    && *pattern_iteration_index < usize::try_from(*pattern_iteration_count)?,
                "battle texture pattern iteration no longer selects its declared record"
            );
            let pattern_address_offset = (*pattern_address_record_index)
                .checked_mul(24)
                .context("battle texture base pattern offset overflow")?;
            let pattern_address = symbol_address(file, pattern_table_symbol)?
                .checked_add(u64::try_from(pattern_address_offset)?)
                .context("battle texture pattern address overflow")?;
            typed_actor_instructions.extend(validate_loaded_address(
                &actor_bytes,
                actor_address,
                *pattern_address_upper_offset,
                *pattern_address_lower_offset,
                GeneralRegister::S3,
                pattern_address,
                "battle texture pattern record address",
            )?);
            typed_actor_instructions.push(require_three_general(
                &actor_bytes,
                actor_address,
                *pattern_argument_offset,
                InstructionForm::Daddu,
                GeneralRegister::A1,
                GeneralRegister::S3,
                GeneralRegister::ZERO,
                "battle texture pattern argument",
            )?);
            typed_actor_instructions.extend([
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *pattern_stride_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::S3,
                    GeneralRegister::S3,
                    24,
                    "battle texture pattern stride",
                )?,
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *pattern_iteration_count_instruction_offset,
                    InstructionForm::Slti,
                    GeneralRegister::V1,
                    GeneralRegister::S1,
                    *pattern_iteration_count,
                    "battle texture pattern iteration count",
                )?,
            ]);
            *pattern_source_bounds
        }
        StylizedArtworkBattleTextureUse::DirectPattern {
            pattern_source_bounds,
            texture_slot_instruction_offset,
            source_x_instruction_offset,
            source_y_instruction_offset,
            width_instruction_offset,
            height_instruction_offset,
        } => {
            typed_actor_instructions.push(if pattern_source_bounds.x == 0 {
                require_three_general(
                    &actor_bytes,
                    actor_address,
                    *source_x_instruction_offset,
                    InstructionForm::Daddu,
                    GeneralRegister::A2,
                    GeneralRegister::ZERO,
                    GeneralRegister::ZERO,
                    "battle direct zero source x",
                )?
            } else {
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *source_x_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::A2,
                    GeneralRegister::ZERO,
                    i16::try_from(pattern_source_bounds.x)?,
                    "battle direct source x",
                )?
            });
            typed_actor_instructions.extend([
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *texture_slot_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::A1,
                    GeneralRegister::ZERO,
                    i16::try_from(consumer.pattern_texture_slot)?,
                    "battle direct texture slot",
                )?,
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *source_y_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::A3,
                    GeneralRegister::ZERO,
                    i16::try_from(pattern_source_bounds.y)?,
                    "battle direct source y",
                )?,
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *width_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::T0,
                    GeneralRegister::ZERO,
                    i16::try_from(pattern_source_bounds.width)?,
                    "battle direct source width",
                )?,
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *height_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::T1,
                    GeneralRegister::ZERO,
                    i16::try_from(pattern_source_bounds.height)?,
                    "battle direct source height",
                )?,
            ]);
            *pattern_source_bounds
        }
        StylizedArtworkBattleTextureUse::ActorTextureBox {
            pattern_source_bounds,
            texture_slot_instruction_offset,
            texture_slot_store_offset,
            source_x_instruction_offset,
            source_y_instruction_offset,
            width_instruction_offset,
            height_instruction_offset,
        } => {
            typed_actor_instructions.extend([
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *texture_slot_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::V1,
                    GeneralRegister::ZERO,
                    i16::try_from(consumer.pattern_texture_slot)?,
                    "battle actor texture slot",
                )?,
                require_memory(
                    &actor_bytes,
                    actor_address,
                    *texture_slot_store_offset,
                    InstructionForm::Sw,
                    GeneralRegister::V1,
                    GeneralRegister::V0,
                    44,
                    "battle actor texture-slot store",
                )?,
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *source_x_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::A1,
                    GeneralRegister::ZERO,
                    i16::try_from(pattern_source_bounds.x)?,
                    "battle box source x",
                )?,
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *source_y_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::A2,
                    GeneralRegister::ZERO,
                    i16::try_from(pattern_source_bounds.y)?,
                    "battle box source y",
                )?,
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *width_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::A3,
                    GeneralRegister::ZERO,
                    i16::try_from(pattern_source_bounds.width)?,
                    "battle box source width",
                )?,
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *height_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::T0,
                    GeneralRegister::ZERO,
                    i16::try_from(pattern_source_bounds.height)?,
                    "battle box source height",
                )?,
            ]);
            *pattern_source_bounds
        }
        StylizedArtworkBattleTextureUse::CharacterPatternRecord {
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
            pattern_record_index_instruction_offset,
            creator_actor_symbol,
            creator_actor_instruction_count,
            creator_function_address_upper_offset,
            creator_function_address_lower_offset,
            creator_call_offset,
            creator_state,
        } => {
            ensure!(
                character_face_index == Some(*character_index),
                "character pattern record disagrees with its face registration route"
            );
            let pointer_addresses =
                read_pointer_table_addresses(file, pattern_pointer_table_symbol)?;
            ensure!(
                pointer_addresses.get(*character_index).copied()
                    == Some(symbol_address(file, pattern_table_symbol)?),
                "character pattern-table pointer changed"
            );
            let (_, pattern_bytes) = read_symbol_bytes(file, pattern_table_symbol)?;
            let patterns = parse_pattern_records(pattern_table_symbol, &pattern_bytes)?;
            let pattern = patterns
                .get(*pattern_record_index)
                .context("character battle pattern record is out of range")?;
            ensure!(
                pattern.image_load_index == consumer.pattern_texture_slot
                    && pattern.source_x == pattern_source_bounds.x
                    && pattern.source_y == pattern_source_bounds.y
                    && pattern.width == pattern_source_bounds.width
                    && pattern.height == pattern_source_bounds.height,
                "character battle pattern record changed"
            );

            typed_actor_instructions.extend(validate_loaded_address(
                &actor_bytes,
                actor_address,
                *pattern_pointer_address_upper_offset,
                *pattern_pointer_address_lower_offset,
                GeneralRegister::V0,
                symbol_address(file, pattern_pointer_table_symbol)?,
                "character pattern pointer table",
            )?);
            typed_actor_instructions.extend([
                require_memory(
                    &actor_bytes,
                    actor_address,
                    0x10,
                    InstructionForm::Lw,
                    GeneralRegister::A1,
                    GeneralRegister::A0,
                    192,
                    "character callout actor state",
                )?,
                require_memory(
                    &actor_bytes,
                    actor_address,
                    *character_selector_offset,
                    InstructionForm::Lhu,
                    GeneralRegister::V1,
                    GeneralRegister::AT,
                    -13_510,
                    "character pattern selector",
                )?,
                require_shift(
                    &actor_bytes,
                    actor_address,
                    *character_selector_scale_offset,
                    InstructionForm::Sll,
                    GeneralRegister::V1,
                    GeneralRegister::V1,
                    2,
                    "character pattern selector scale",
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
                    "character pattern cell initializer",
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
                require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    *pattern_record_index_instruction_offset,
                    InstructionForm::Addiu,
                    GeneralRegister::A1,
                    GeneralRegister::ZERO,
                    i16::try_from(*pattern_record_index)?,
                    "selected character pattern record",
                )?,
            ]);

            let (state_value, state_branch_offset, state_branch_right) = match creator_state {
                StylizedArtworkCreatedActorState::ZeroInitialized => {
                    let (initializer_address, initializer_bytes) =
                        read_symbol_bytes(file, "Act_SetBottom")?;
                    ensure!(
                        disassemble_symbol(executable, "Act_SetBottom")?
                            .instructions
                            .len()
                            == 63,
                        "actor initializer instruction population changed"
                    );
                    typed_creator_instructions.extend([
                        require_memory(
                            &initializer_bytes,
                            initializer_address,
                            0x54,
                            InstructionForm::Sw,
                            GeneralRegister::ZERO,
                            GeneralRegister::S0,
                            192,
                            "created actor zero state",
                        )?,
                        require_three_general(
                            &initializer_bytes,
                            initializer_address,
                            0xcc,
                            InstructionForm::Daddu,
                            GeneralRegister::V0,
                            GeneralRegister::S0,
                            GeneralRegister::ZERO,
                            "created actor return value",
                        )?,
                    ]);
                    (0_i16, 0x1c, GeneralRegister::ZERO)
                }
                StylizedArtworkCreatedActorState::Explicit { value, .. } => {
                    ensure!(*value == 1, "unsupported character callout actor state");
                    (*value, 0x14, GeneralRegister::V1)
                }
            };
            let state_block_offset = pattern_pointer_address_upper_offset
                .checked_sub(8)
                .context("character state block offset underflow")?;
            if state_value == 1 {
                typed_actor_instructions.push(require_signed_immediate(
                    &actor_bytes,
                    actor_address,
                    0x04,
                    InstructionForm::Addiu,
                    GeneralRegister::V1,
                    GeneralRegister::ZERO,
                    1,
                    "character callout state selector",
                )?);
            }
            typed_actor_instructions.push(require_branch_two(
                &actor_bytes,
                actor_address,
                state_branch_offset,
                InstructionForm::Beq,
                GeneralRegister::A1,
                state_branch_right,
                actor_address + u64::try_from(state_block_offset)?,
                "character callout state dispatch",
            )?);

            ensure!(
                disassemble_symbol(executable, creator_actor_symbol)?
                    .instructions
                    .len()
                    == *creator_actor_instruction_count,
                "character callout creator instruction population changed"
            );
            let (creator_address, creator_bytes) = read_symbol_bytes(file, creator_actor_symbol)?;
            typed_creator_instructions.extend(validate_loaded_address(
                &creator_bytes,
                creator_address,
                *creator_function_address_upper_offset,
                *creator_function_address_lower_offset,
                GeneralRegister::A0,
                symbol_address(file, &consumer.actor_symbol)?,
                "character callout actor function",
            )?);
            typed_creator_instructions.push(validate_jump_target(
                &creator_bytes,
                creator_address,
                *creator_call_offset,
                symbol_address(file, "Act_SetBottom")?,
                "character callout actor creation call",
            )?);
            if let StylizedArtworkCreatedActorState::Explicit {
                value,
                value_instruction_offset,
                state_store_offset,
            } = creator_state
            {
                typed_creator_instructions.extend([
                    require_signed_immediate(
                        &creator_bytes,
                        creator_address,
                        *value_instruction_offset,
                        InstructionForm::Addiu,
                        GeneralRegister::A1,
                        GeneralRegister::ZERO,
                        *value,
                        "created actor explicit state",
                    )?,
                    require_memory(
                        &creator_bytes,
                        creator_address,
                        *state_store_offset,
                        InstructionForm::Sw,
                        GeneralRegister::A1,
                        GeneralRegister::V0,
                        192,
                        "created actor state store",
                    )?,
                ]);
            }
            *pattern_source_bounds
        }
        texture_use @ StylizedArtworkBattleTextureUse::CharacterActionPatternRecord { .. } => {
            let evidence = validate_character_action_pattern_record(
                file,
                executable,
                consumer,
                character_face_index,
                texture_use,
            )?;
            typed_actor_instructions.extend(evidence.typed_actor_instructions);
            typed_creator_instructions.extend(evidence.typed_creator_instructions);
            evidence.pattern_source_bounds
        }
        texture_use @ StylizedArtworkBattleTextureUse::FaceConditionPatternRecord { .. } => {
            let evidence = validate_face_condition_pattern_record(
                file,
                executable,
                consumer,
                character_face_index,
                texture_use,
            )?;
            typed_actor_instructions.extend(evidence.typed_actor_instructions);
            typed_creator_instructions.extend(evidence.typed_creator_instructions);
            evidence.pattern_source_bounds
        }
        StylizedArtworkBattleTextureUse::FaceActionPatternRecord {
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
            face_state_pointer_table_symbol,
            face_state_index,
            face_state_table_symbol,
            face_state_entry_index,
            face_action_symbol,
            face_action_pattern_indices,
        } => {
            ensure!(
                character_face_index == Some(*character_index),
                "face action pattern disagrees with its face registration route"
            );
            let pattern_pointer_addresses =
                read_pointer_table_addresses(file, pattern_pointer_table_symbol)?;
            ensure!(
                pattern_pointer_addresses.get(*character_index).copied()
                    == Some(symbol_address(file, pattern_table_symbol)?),
                "face action pattern-table pointer changed"
            );
            let (_, pattern_bytes) = read_symbol_bytes(file, pattern_table_symbol)?;
            let patterns = parse_pattern_records(pattern_table_symbol, &pattern_bytes)?;
            let pattern = patterns
                .get(*pattern_record_index)
                .context("face action pattern record is out of range")?;
            ensure!(
                pattern.image_load_index == consumer.pattern_texture_slot
                    && pattern.source_x == pattern_source_bounds.x
                    && pattern.source_y == pattern_source_bounds.y
                    && pattern.width == pattern_source_bounds.width
                    && pattern.height == pattern_source_bounds.height,
                "face action pattern record changed"
            );

            typed_actor_instructions.extend(validate_loaded_address(
                &actor_bytes,
                actor_address,
                *pattern_pointer_address_upper_offset,
                *pattern_pointer_address_lower_offset,
                GeneralRegister::V0,
                symbol_address(file, pattern_pointer_table_symbol)?,
                "face pattern pointer table",
            )?);
            typed_actor_instructions.extend([
                require_memory(
                    &actor_bytes,
                    actor_address,
                    *character_selector_offset,
                    InstructionForm::Lhu,
                    GeneralRegister::V1,
                    GeneralRegister::AT,
                    -13_510,
                    "face pattern character selector",
                )?,
                require_shift(
                    &actor_bytes,
                    actor_address,
                    *character_selector_scale_offset,
                    InstructionForm::Sll,
                    GeneralRegister::V1,
                    GeneralRegister::V1,
                    2,
                    "face pattern character selector scale",
                )?,
                require_three_general(
                    &actor_bytes,
                    actor_address,
                    *selected_pattern_table_offset,
                    InstructionForm::Addu,
                    GeneralRegister::V0,
                    GeneralRegister::V0,
                    GeneralRegister::V1,
                    "selected face pattern table",
                )?,
                require_memory(
                    &actor_bytes,
                    actor_address,
                    pattern_initializer_call_offset
                        .checked_sub(4)
                        .context("face pattern pointer load offset underflow")?,
                    InstructionForm::Lw,
                    GeneralRegister::A1,
                    GeneralRegister::V0,
                    0,
                    "selected face pattern-table pointer",
                )?,
                validate_jump_target(
                    &actor_bytes,
                    actor_address,
                    *pattern_initializer_call_offset,
                    symbol_address(file, "Act_CellInit")?,
                    "face pattern cell initializer",
                )?,
            ]);

            let state_pointer_addresses =
                read_pointer_table_addresses(file, face_state_pointer_table_symbol)?;
            ensure!(
                state_pointer_addresses.get(*face_state_index).copied()
                    == Some(symbol_address(file, face_state_table_symbol)?),
                "face state-table pointer changed"
            );
            validate_face_state_action(
                file,
                face_state_table_symbol,
                *face_state_entry_index,
                face_action_symbol,
            )?;
            validate_face_action_pattern_indices(
                file,
                face_action_symbol,
                face_action_pattern_indices,
            )?;
            ensure!(
                face_action_pattern_indices.contains(pattern_record_index),
                "face action no longer selects the declared pattern record"
            );
            *pattern_source_bounds
        }
        StylizedArtworkBattleTextureUse::SourceOnlyRegion {
            source_bounds,
            pattern_pointer_table_symbol,
            character_index,
            character_pattern_table_symbol,
            additional_pattern_table_symbols,
        } => {
            ensure!(
                character_face_index == Some(*character_index),
                "source-only face region disagrees with its registration route"
            );
            let pattern_pointer_addresses =
                read_pointer_table_addresses(file, pattern_pointer_table_symbol)?;
            ensure!(
                pattern_pointer_addresses.get(*character_index).copied()
                    == Some(symbol_address(file, character_pattern_table_symbol)?),
                "source-only face pattern-table pointer changed"
            );
            let mut pattern_table_symbols = vec![character_pattern_table_symbol.as_str()];
            pattern_table_symbols
                .extend(additional_pattern_table_symbols.iter().map(String::as_str));
            ensure!(
                pattern_table_symbols
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len()
                    == pattern_table_symbols.len()
                    && matches!(
                        additional_pattern_table_symbols.as_slice(),
                        [symbol] if symbol == "FaceActPtnTbl"
                    ),
                "source-only face pattern-table coverage changed"
            );
            for pattern_table_symbol in pattern_table_symbols {
                let (_, pattern_bytes) = read_symbol_bytes(file, pattern_table_symbol)?;
                let patterns = parse_pattern_records(pattern_table_symbol, &pattern_bytes)?;
                ensure!(
                    patterns
                        .iter()
                        .filter(|pattern| {
                            pattern.image_load_index == consumer.pattern_texture_slot
                        })
                        .all(|pattern| {
                            !pixel_bounds_overlap(
                                PixelBounds {
                                    x: pattern.source_x,
                                    y: pattern.source_y,
                                    width: pattern.width,
                                    height: pattern.height,
                                },
                                *source_bounds,
                            )
                        }),
                    "source-only face region now overlaps a selected pattern"
                );
            }
            *source_bounds
        }
    };
    ensure_bounds_contains(
        pattern_source_bounds,
        region.bounds,
        "battle Japanese region exceeds its consumed source rectangle",
    )?;
    typed_actor_instructions.sort_by(|left, right| left.address.cmp(&right.address));
    typed_creator_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkBattleTextureConsumerValidation {
        load_route: consumer.load_route.clone(),
        pattern_texture_slot: consumer.pattern_texture_slot,
        actor_symbol: consumer.actor_symbol.clone(),
        pattern_setter_symbol: consumer.pattern_setter_symbol.clone(),
        texture_use: consumer.texture_use.clone(),
        typed_actor_instructions,
        typed_creator_instructions,
        runtime_status: if matches!(
            &consumer.texture_use,
            StylizedArtworkBattleTextureUse::SourceOnlyRegion { .. }
        ) {
            "source sheet only; no selected static face pattern; exact overlay not observed"
        } else {
            "not observed for this exact artwork overlay"
        },
    })
}
