use super::*;

pub(in crate::graphics_translation::stylized_artwork) fn validate_scrolling_tile_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkScrollingTileConsumer,
    region: &super::GraphicsTextRegion,
) -> Result<StylizedArtworkScrollingTileConsumerValidation> {
    ensure!(
        !consumer.load_routes.is_empty()
            && consumer.load_routes.len() == consumer.registration_routes.len()
            && consumer.positions.len() >= 4,
        "scrolling stylized artwork consumer routes or positions are incomplete"
    );
    let primary = &consumer.load_routes[0];
    let mut load_tables = BTreeSet::new();
    for route in &consumer.load_routes {
        ensure!(
            load_tables.insert(route.load_table_symbol.as_str())
                && route.image_file_record == primary.image_file_record
                && route.palette_file_record == primary.palette_file_record,
            "scrolling stylized artwork load routes are duplicated or disagree on the physical asset"
        );
        let (_, load_bytes) = read_symbol_bytes(file, &route.load_table_symbol)?;
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
                .is_some_and(|record| record.file_record == route.image_file_record)
                && palettes
                    .get(route.palette_slot)
                    .is_some_and(|record| record.file_record == route.palette_file_record),
            "scrolling stylized artwork load-table route changed: {}",
            route.load_table_symbol
        );
    }

    let mut registration_sets = BTreeSet::new();
    let mut registered_load_tables = BTreeSet::new();
    for route in &consumer.registration_routes {
        ensure!(
            registration_sets.insert(route.registration_set_symbol.as_str())
                && registered_load_tables.insert(route.load_table_symbol.as_str()),
            "scrolling stylized artwork registration routes are duplicated"
        );
        let load_route = consumer
            .load_routes
            .iter()
            .find(|candidate| candidate.load_table_symbol == route.load_table_symbol)
            .with_context(|| {
                format!(
                    "scrolling registration route has no load table: {}",
                    route.load_table_symbol
                )
            })?;
        ensure!(
            route.texture_slot == consumer.pattern_texture_slot
                && route.image_load_index == load_route.image_load_index
                && route.palette_slot == load_route.palette_slot,
            "scrolling stylized artwork registration route does not map the pattern slot to its source asset"
        );
        let (_, registration_bytes) = read_symbol_bytes(file, &route.registration_set_symbol)?;
        let pairs =
            parse_texture_registration_pairs(&route.registration_set_symbol, &registration_bytes)?;
        ensure!(
            pairs.get(route.texture_slot).is_some_and(|pair| {
                pair.image_load_index == route.image_load_index
                    && pair.palette_slot == route.palette_slot
            }),
            "scrolling stylized artwork registration pair changed: {} slot {}",
            route.registration_set_symbol,
            route.texture_slot
        );
    }
    ensure!(
        load_tables == registered_load_tables,
        "scrolling stylized artwork load and registration route coverage differs"
    );

    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let pattern = patterns
        .get(consumer.pattern_record_index)
        .context("scrolling stylized artwork pattern record is out of range")?;
    ensure!(
        pattern.image_load_index == consumer.pattern_texture_slot
            && pattern.source_x == consumer.pattern_source_bounds.x
            && pattern.source_y == consumer.pattern_source_bounds.y
            && pattern.width == consumer.pattern_source_bounds.width
            && pattern.height == consumer.pattern_source_bounds.height,
        "scrolling stylized artwork pattern record changed"
    );
    ensure_bounds_contains(
        consumer.pattern_source_bounds,
        region.bounds,
        "scrolling stylized artwork Japanese region exceeds its source tile",
    )?;

    let (_, position_bytes) = read_symbol_bytes(file, &consumer.position_table_symbol)?;
    ensure!(
        position_bytes.len() == consumer.positions.len() * 4,
        "scrolling stylized artwork position-table length changed"
    );
    let positions = position_bytes
        .chunks_exact(4)
        .map(|record| StylizedArtworkScreenPoint {
            x: i16::from_le_bytes(record[0..2].try_into().expect("two-byte x position")),
            y: i16::from_le_bytes(record[2..4].try_into().expect("two-byte y position")),
        })
        .collect::<Vec<_>>();
    ensure!(
        positions == consumer.positions,
        "scrolling stylized artwork positions changed"
    );
    validate_scrolling_tile_grid(consumer)?;

    ensure!(
        disassemble_symbol(executable, &consumer.actor_symbol)?
            .instructions
            .len()
            == consumer.actor_instruction_count,
        "scrolling stylized artwork actor instruction population changed"
    );
    let (actor_address, actor_bytes) = read_symbol_bytes(file, &consumer.actor_symbol)?;
    let pattern_record_offset = consumer
        .pattern_record_index
        .checked_mul(24)
        .context("scrolling stylized artwork pattern offset overflow")?;
    let pattern_record_address = symbol_address(file, &consumer.pattern_table_symbol)?
        .checked_add(u64::try_from(pattern_record_offset)?)
        .context("scrolling stylized artwork pattern address overflow")?;
    let mut typed_actor_instructions = validate_loaded_address(
        &actor_bytes,
        actor_address,
        0x30,
        0x38,
        GeneralRegister::A1,
        pattern_record_address,
        "scrolling tile pattern record",
    )?;
    typed_actor_instructions.extend([
        validate_jump_target(
            &actor_bytes,
            actor_address,
            0x34,
            symbol_address(file, &consumer.pattern_setter_symbol)?,
            "scrolling tile pattern setter",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            0x60,
            InstructionForm::Addiu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "scrolling tile horizontal movement",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            0x70,
            InstructionForm::Addiu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "scrolling tile vertical movement",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            0xd4,
            InstructionForm::Addiu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            -i16::try_from(consumer.horizontal_wrap_distance)?,
            "scrolling tile horizontal wrap",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            0x114,
            InstructionForm::Addiu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            -i16::try_from(consumer.vertical_wrap_distance)?,
            "scrolling tile vertical wrap",
        )?,
    ]);
    typed_actor_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkScrollingTileConsumerValidation {
        load_routes: consumer.load_routes.clone(),
        registration_routes: consumer.registration_routes.clone(),
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        pattern_record_index: consumer.pattern_record_index,
        pattern_texture_slot: consumer.pattern_texture_slot,
        pattern_source_bounds: consumer.pattern_source_bounds,
        actor_symbol: consumer.actor_symbol.clone(),
        position_table_symbol: consumer.position_table_symbol.clone(),
        positions,
        horizontal_wrap_distance: consumer.horizontal_wrap_distance,
        vertical_wrap_distance: consumer.vertical_wrap_distance,
        typed_actor_instructions,
        runtime_status: "not observed for this exact artwork overlay",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_mode_selection_states_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    native_placement: PixelBounds,
    consumer: &StylizedArtworkModeSelectionStatesConsumer,
    region: &super::GraphicsTextRegion,
) -> Result<StylizedArtworkModeSelectionStatesConsumerValidation> {
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
            .is_some_and(|record| record.file_record == consumer.load_route.image_file_record)
            && palettes
                .get(consumer.load_route.palette_slot)
                .is_some_and(|record| {
                    record.file_record == consumer.load_route.palette_file_record
                })
            && consumer.load_route.image_load_index == consumer.pattern_texture_slot
            && consumer.load_route.palette_slot == consumer.pattern_texture_slot,
        "mode-selection artwork load-table route changed"
    );

    ensure!(
        disassemble_symbol(executable, &consumer.registration_actor_symbol)?
            .instructions
            .len()
            == consumer.registration_actor_instruction_count,
        "mode-selection registration actor instruction population changed"
    );
    let (registration_address, registration_bytes) =
        read_symbol_bytes(file, &consumer.registration_actor_symbol)?;
    let mut typed_registration_instructions = validate_loaded_address(
        &registration_bytes,
        registration_address,
        consumer.load_table_address_upper_offset,
        consumer.load_table_address_lower_offset,
        GeneralRegister::A1,
        symbol_address(file, &consumer.load_route.load_table_symbol)?,
        "mode-selection load table",
    )?;
    typed_registration_instructions.push(validate_jump_target(
        &registration_bytes,
        registration_address,
        consumer.registration_call_offset,
        symbol_address(file, &consumer.registration_function_symbol)?,
        "mode-selection texture registration",
    )?);
    typed_registration_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (_, pointer_bytes) = read_symbol_bytes(file, &consumer.pattern_pointer_table_symbol)?;
    ensure!(
        pointer_bytes.len().is_multiple_of(4),
        "mode-selection pattern pointer table is not u32-aligned"
    );
    let pattern_pointer = read_u32_at(
        &pointer_bytes,
        consumer
            .pattern_pointer_table_index
            .checked_mul(4)
            .context("mode-selection pattern pointer offset overflow")?,
    )?;
    ensure!(
        u64::from(pattern_pointer) == symbol_address(file, &consumer.pattern_table_symbol)?,
        "mode-selection mode-to-pattern pointer changed"
    );

    ensure!(
        consumer.state_pattern_record_indices == [0, 1, 2]
            && consumer.state_pattern_source_bounds.len() == 3
            && consumer.state_source_mask_bounds.len() == 3
            && consumer.state_artwork_bounds.len() == 3
            && consumer.state_artwork_bounds[0] == native_placement
            && consumer.blank_pattern_record_index == 3,
        "mode-selection state population or primary placement changed"
    );
    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    ensure!(
        patterns.len() == 4,
        "mode-selection pattern table no longer has three visible states and one blank frame"
    );
    for (((record_index, expected_bounds), source_mask_bounds), artwork_bounds) in consumer
        .state_pattern_record_indices
        .iter()
        .zip(&consumer.state_pattern_source_bounds)
        .zip(&consumer.state_source_mask_bounds)
        .zip(&consumer.state_artwork_bounds)
    {
        let pattern = patterns
            .get(*record_index)
            .context("mode-selection state pattern record is out of range")?;
        ensure!(
            pattern.image_load_index == consumer.pattern_texture_slot
                && pattern.source_x == expected_bounds.x
                && pattern.source_y == expected_bounds.y
                && pattern.width == expected_bounds.width
                && pattern.height == expected_bounds.height,
            "mode-selection state pattern record changed"
        );
        ensure_bounds_contains(
            *expected_bounds,
            *source_mask_bounds,
            "mode-selection source-mask footprint exceeds its consumed state pattern",
        )?;
        ensure_bounds_contains(
            *expected_bounds,
            *artwork_bounds,
            "mode-selection artwork footprint exceeds its consumed state pattern",
        )?;
        ensure_bounds_contains(
            region.bounds,
            *source_mask_bounds,
            "mode-selection source-mask footprint exceeds its Japanese region",
        )?;
        ensure_bounds_contains(
            region.artwork_composition_bounds.unwrap_or(region.bounds),
            *artwork_bounds,
            "mode-selection artwork footprint exceeds its authored composition",
        )?;
    }
    let blank = patterns
        .get(consumer.blank_pattern_record_index)
        .context("mode-selection blank pattern record is out of range")?;
    ensure!(
        blank.image_load_index == consumer.pattern_texture_slot
            && blank.source_x == consumer.blank_pattern_source_bounds.x
            && blank.source_y == consumer.blank_pattern_source_bounds.y
            && blank.width == consumer.blank_pattern_source_bounds.width
            && blank.height == consumer.blank_pattern_source_bounds.height,
        "mode-selection blank pattern record changed"
    );

    ensure!(
        consumer.animation_pattern_indices == [0, 1, 2, 3] && consumer.animation_frame_duration > 0,
        "mode-selection animation state order changed"
    );
    let (_, animation_bytes) = read_symbol_bytes(file, &consumer.animation_table_symbol)?;
    ensure!(
        animation_bytes.len() == (consumer.animation_pattern_indices.len() + 1) * 4,
        "mode-selection animation table length changed"
    );
    for (frame, expected_pattern) in animation_bytes
        .chunks_exact(4)
        .take(consumer.animation_pattern_indices.len())
        .zip(&consumer.animation_pattern_indices)
    {
        ensure!(
            u16::from_le_bytes([frame[0], frame[1]]) == *expected_pattern
                && u16::from_le_bytes([frame[2], frame[3]]) == consumer.animation_frame_duration,
            "mode-selection animation frame changed"
        );
    }
    let terminator = &animation_bytes[animation_bytes.len() - 4..];
    ensure!(
        u16::from_le_bytes([terminator[0], terminator[1]]) == u16::MAX
            && u16::from_le_bytes([terminator[2], terminator[3]]) == 0,
        "mode-selection animation terminator changed"
    );

    ensure!(
        disassemble_symbol(executable, &consumer.actor_symbol)?
            .instructions
            .len()
            == consumer.actor_instruction_count,
        "mode-selection actor instruction population changed"
    );
    let (actor_address, actor_bytes) = read_symbol_bytes(file, &consumer.actor_symbol)?;
    let mut typed_actor_instructions = validate_loaded_address(
        &actor_bytes,
        actor_address,
        0x3c,
        0x40,
        GeneralRegister::V0,
        symbol_address(file, &consumer.pattern_pointer_table_symbol)?,
        "mode-selection pattern pointer table",
    )?;
    typed_actor_instructions.extend([
        require_memory(
            &actor_bytes,
            actor_address,
            0x38,
            InstructionForm::Lw,
            GeneralRegister::V1,
            GeneralRegister::S0,
            consumer.mode_index_offset,
            "mode-selection mode index",
        )?,
        require_shift(
            &actor_bytes,
            actor_address,
            0x44,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "mode-selection pointer index stride",
        )?,
        require_three_general(
            &actor_bytes,
            actor_address,
            0x48,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "mode-selection selected pattern table pointer",
        )?,
        validate_jump_target(
            &actor_bytes,
            actor_address,
            0x4c,
            symbol_address(file, &consumer.pattern_setter_symbol)?,
            "mode-selection pattern setter",
        )?,
        require_memory(
            &actor_bytes,
            actor_address,
            0x50,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "mode-selection selected pattern table load",
        )?,
    ]);
    typed_actor_instructions.extend(validate_loaded_address(
        &actor_bytes,
        actor_address,
        consumer.animation_address_upper_offset,
        consumer.animation_address_lower_offset,
        GeneralRegister::A1,
        symbol_address(file, &consumer.animation_table_symbol)?,
        "mode-selection animation table",
    )?);
    typed_actor_instructions.push(validate_jump_target(
        &actor_bytes,
        actor_address,
        consumer.animation_call_offset,
        symbol_address(file, &consumer.animation_initializer_symbol)?,
        "mode-selection animation initializer",
    )?);
    typed_actor_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkModeSelectionStatesConsumerValidation {
        load_route: consumer.load_route.clone(),
        pattern_texture_slot: consumer.pattern_texture_slot,
        registration_actor_symbol: consumer.registration_actor_symbol.clone(),
        registration_function_symbol: consumer.registration_function_symbol.clone(),
        pattern_pointer_table_symbol: consumer.pattern_pointer_table_symbol.clone(),
        pattern_pointer_table_index: consumer.pattern_pointer_table_index,
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        state_pattern_record_indices: consumer.state_pattern_record_indices.clone(),
        state_pattern_source_bounds: consumer.state_pattern_source_bounds.clone(),
        state_source_mask_bounds: consumer.state_source_mask_bounds.clone(),
        state_artwork_bounds: consumer.state_artwork_bounds.clone(),
        blank_pattern_record_index: consumer.blank_pattern_record_index,
        blank_pattern_source_bounds: consumer.blank_pattern_source_bounds,
        actor_symbol: consumer.actor_symbol.clone(),
        pattern_setter_symbol: consumer.pattern_setter_symbol.clone(),
        mode_index_offset: consumer.mode_index_offset,
        animation_table_symbol: consumer.animation_table_symbol.clone(),
        animation_pattern_indices: consumer.animation_pattern_indices.clone(),
        animation_frame_duration: consumer.animation_frame_duration,
        animation_initializer_symbol: consumer.animation_initializer_symbol.clone(),
        typed_registration_instructions,
        typed_actor_instructions,
        runtime_status: "not observed for this exact artwork overlay",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_title_branding_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    native_placement: PixelBounds,
    consumer: &StylizedArtworkTitleBrandingConsumer,
    region: &super::GraphicsTextRegion,
) -> Result<StylizedArtworkTitleBrandingConsumerValidation> {
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
            .is_some_and(|record| record.file_record == consumer.load_route.image_file_record)
            && palettes
                .get(consumer.load_route.palette_slot)
                .is_some_and(|record| {
                    record.file_record == consumer.load_route.palette_file_record
                }),
        "title-branding artwork load-table route changed"
    );
    ensure!(
        !consumer.registration_routes.is_empty(),
        "title-branding artwork has no texture registration route"
    );
    let mut registration_symbols = BTreeSet::new();
    for route in &consumer.registration_routes {
        ensure!(
            registration_symbols.insert(route.registration_set_symbol.as_str())
                && route.load_table_symbol == consumer.load_route.load_table_symbol
                && route.texture_slot == consumer.pattern_texture_slot
                && route.image_load_index == consumer.load_route.image_load_index
                && route.palette_slot == consumer.load_route.palette_slot,
            "title-branding registration route no longer maps the pattern texture slot"
        );
        let (_, registration_bytes) = read_symbol_bytes(file, &route.registration_set_symbol)?;
        let pairs =
            parse_texture_registration_pairs(&route.registration_set_symbol, &registration_bytes)?;
        ensure!(
            pairs.get(route.texture_slot).is_some_and(|pair| {
                pair.image_load_index == route.image_load_index
                    && pair.palette_slot == route.palette_slot
            }),
            "title-branding registration pair changed"
        );
    }

    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let mut pattern_indices = BTreeSet::new();
    validate_title_pattern_slice(
        &patterns,
        consumer.pattern_texture_slot,
        &consumer.body_pattern,
        "complete title body",
    )?;
    pattern_indices.insert(consumer.body_pattern.pattern_record_index);
    ensure!(
        !consumer.body_fragment_patterns.is_empty(),
        "title-branding artwork has no split-animation body fragments"
    );
    let mut stored_body_text = String::new();
    for fragment in &consumer.body_fragment_patterns {
        ensure!(
            !fragment.stored_korean_text.is_empty()
                && pattern_indices.insert(fragment.pattern_record_index),
            "title-branding pattern records are duplicated"
        );
        let pattern_slice = StylizedArtworkPatternSlice {
            pattern_record_index: fragment.pattern_record_index,
            pattern_source_bounds: fragment.pattern_source_bounds,
        };
        validate_title_pattern_slice(
            &patterns,
            consumer.pattern_texture_slot,
            &pattern_slice,
            "split-animation title body fragment",
        )?;
        ensure_bounds_contains(
            consumer.body_pattern.pattern_source_bounds,
            fragment.pattern_source_bounds,
            "title-branding body fragment exceeds the complete body pattern",
        )?;
        stored_body_text.push_str(&fragment.stored_korean_text);
    }
    ensure!(
        stored_body_text == consumer.stored_body_text
            && consumer.assembled_body_text == consumer.stored_body_text.repeat(2),
        "title-branding stored and assembled Korean text disagree with its fragment texts"
    );

    ensure!(
        consumer.sequence_symbol == "TitleDemo"
            && consumer.sequence_opcode == 0x10
            && consumer.sequence_command_words == 7
            && consumer.pattern_setter_symbol == "Act_SetPtn"
            && consumer.assembly_commands.len() == consumer.body_fragment_patterns.len() * 2,
        "title-branding repeat assembly header changed"
    );
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
            .get(consumer.sequence_opcode)
            .is_some_and(|words| *words == consumer.sequence_command_words),
        "title-branding sequence opcode size changed"
    );
    let (_, sequence_bytes) = read_symbol_bytes(file, &consumer.sequence_symbol)?;
    let mut actor_indices = BTreeSet::new();
    let mut assembled_body_text = String::new();
    let command_byte_length = consumer
        .sequence_command_words
        .checked_mul(2)
        .context("title-branding command length overflow")?;
    for (command_index, declared) in consumer.assembly_commands.iter().enumerate() {
        let expected_fragment =
            &consumer.body_fragment_patterns[command_index % consumer.body_fragment_patterns.len()];
        ensure!(
            declared.pattern_record_index == expected_fragment.pattern_record_index
                && actor_indices.insert(declared.sequence_actor_index)
                && (command_index == 0
                    || declared.sequence_command_offset
                        == consumer.assembly_commands[command_index - 1]
                            .sequence_command_offset
                            .checked_add(command_byte_length)
                            .context("title-branding command offset overflow")?),
            "title-branding commands no longer repeat the stored fragment order"
        );
        let command = find_sequence_command(
            &consumer.sequence_symbol,
            &sequence_bytes,
            &command_sizes,
            declared.sequence_command_offset,
        )?;
        ensure!(
            command.len() == command_byte_length
                && usize::from(u16::from_le_bytes(command[0..2].try_into()?))
                    == consumer.sequence_opcode
                && u16::from_le_bytes(command[2..4].try_into()?) == declared.sequence_actor_index
                && u16::from_le_bytes(command[4..6].try_into()?) == declared.sequence_flags
                && i16::from_le_bytes(command[6..8].try_into()?)
                    == declared.pattern_screen_origin.x
                && i16::from_le_bytes(command[8..10].try_into()?)
                    == declared.pattern_screen_origin.y
                && u16::from_le_bytes(command[10..12].try_into()?) == declared.pattern_depth
                && usize::from(u16::from_le_bytes(command[12..14].try_into()?))
                    == declared.pattern_record_index,
            "title-branding TitleDemo command fields changed"
        );
        assembled_body_text.push_str(&expected_fragment.stored_korean_text);
    }
    ensure!(
        assembled_body_text == consumer.assembled_body_text,
        "title-branding TitleDemo commands do not assemble the declared Korean body"
    );
    let typed_decoder_instructions = validate_title_sequence_decoder(file, executable, consumer)?;

    for (slice, label) in [
        (&consumer.suffix_pattern, "Japanese title suffix"),
        (
            &consumer.protected_english_pattern,
            "protected English title badge",
        ),
    ] {
        ensure!(
            pattern_indices.insert(slice.pattern_record_index),
            "title-branding pattern records are duplicated"
        );
        validate_title_pattern_slice(&patterns, consumer.pattern_texture_slot, slice, label)?;
    }

    ensure_bounds_contains(
        consumer.body_pattern.pattern_source_bounds,
        native_placement,
        "title-branding native lettering escapes the complete body pattern",
    )?;
    ensure!(
        region.source_restoration_bounds
            == [
                consumer.body_pattern.pattern_source_bounds,
                consumer.suffix_pattern.pattern_source_bounds,
            ],
        "title-branding source restoration does not match the body and suffix patterns"
    );
    ensure!(
        !pixel_bounds_overlap(
            consumer.body_pattern.pattern_source_bounds,
            consumer.suffix_pattern.pattern_source_bounds,
        ) && !pixel_bounds_overlap(
            consumer.body_pattern.pattern_source_bounds,
            consumer.protected_english_pattern.pattern_source_bounds,
        ) && !pixel_bounds_overlap(
            consumer.suffix_pattern.pattern_source_bounds,
            consumer.protected_english_pattern.pattern_source_bounds,
        ),
        "title-branding body, suffix, and protected English patterns overlap"
    );

    Ok(StylizedArtworkTitleBrandingConsumerValidation {
        load_route: consumer.load_route.clone(),
        registration_routes: consumer.registration_routes.clone(),
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        pattern_texture_slot: consumer.pattern_texture_slot,
        body_pattern: consumer.body_pattern.clone(),
        stored_body_text: consumer.stored_body_text.clone(),
        assembled_body_text: consumer.assembled_body_text.clone(),
        body_fragment_patterns: consumer.body_fragment_patterns.clone(),
        sequence_symbol: consumer.sequence_symbol.clone(),
        sequence_opcode: consumer.sequence_opcode,
        sequence_command_words: consumer.sequence_command_words,
        sequence_case_address: consumer.sequence_case_address,
        pattern_setter_symbol: consumer.pattern_setter_symbol.clone(),
        assembly_commands: consumer.assembly_commands.clone(),
        suffix_pattern: consumer.suffix_pattern.clone(),
        protected_english_pattern: consumer.protected_english_pattern.clone(),
        typed_decoder_instructions,
        runtime_status: "not observed for this exact artwork overlay",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_title_pattern_slice(
    patterns: &[TexturePatternRecord],
    pattern_texture_slot: usize,
    slice: &StylizedArtworkPatternSlice,
    label: &str,
) -> Result<()> {
    let pattern = patterns
        .get(slice.pattern_record_index)
        .with_context(|| format!("{label} pattern record is out of range"))?;
    ensure!(
        pattern.image_load_index == pattern_texture_slot
            && pattern.source_x == slice.pattern_source_bounds.x
            && pattern.source_y == slice.pattern_source_bounds.y
            && pattern.width == slice.pattern_source_bounds.width
            && pattern.height == slice.pattern_source_bounds.height,
        "{label} pattern record changed"
    );
    Ok(())
}

pub(in crate::graphics_translation::stylized_artwork) fn pixel_bounds_overlap(
    left: PixelBounds,
    right: PixelBounds,
) -> bool {
    u64::from(left.x) < u64::from(right.x) + u64::from(right.width)
        && u64::from(right.x) < u64::from(left.x) + u64::from(left.width)
        && u64::from(left.y) < u64::from(right.y) + u64::from(right.height)
        && u64::from(right.y) < u64::from(left.y) + u64::from(left.height)
}

pub(in crate::graphics_translation::stylized_artwork) fn clip_overlay_to_consumer_patterns(
    overlay: &mut [u8],
    claimed_overlay: &mut [bool],
    width: usize,
    height: usize,
    consumer: &StylizedArtworkConsumer,
) -> Result<()> {
    ensure!(
        overlay.len() == width * height * 4 && claimed_overlay.len() == width * height,
        "consumer overlay clipping dimensions changed"
    );
    let StylizedArtworkConsumer::TitleBranding(title) = consumer else {
        return Ok(());
    };
    let body = title.body_pattern.pattern_source_bounds;
    ensure!(
        u64::from(body.x) + u64::from(body.width) <= u64::try_from(width)?
            && u64::from(body.y) + u64::from(body.height) <= u64::try_from(height)?,
        "title-branding body pattern exceeds its source surface"
    );
    for y in body.y..body.y + body.height {
        for x in body.x..body.x + body.width {
            if title
                .body_fragment_patterns
                .iter()
                .any(|fragment| pixel_is_inside(fragment.pattern_source_bounds, x, y))
            {
                continue;
            }
            let pixel = usize::try_from(y)? * width + usize::try_from(x)?;
            overlay[pixel * 4..pixel * 4 + 4].fill(0);
            claimed_overlay[pixel] = false;
        }
    }
    Ok(())
}

pub(in crate::graphics_translation::stylized_artwork) fn pixel_is_inside(
    bounds: PixelBounds,
    x: u32,
    y: u32,
) -> bool {
    x >= bounds.x && x < bounds.x + bounds.width && y >= bounds.y && y < bounds.y + bounds.height
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_source_mask_footprints(
    asset: &HashedAsset,
    width: usize,
    height: usize,
    footprints: &[PixelBounds],
) -> Result<()> {
    verify_hashed_asset(asset)?;
    let mask = read_binary_mask(
        Path::new(&asset.path),
        width,
        height,
        "mode-selection source",
    )?;
    ensure!(
        !footprints.is_empty(),
        "mode-selection source has no state footprints"
    );
    for (index, left) in footprints.iter().enumerate() {
        for right in footprints.iter().skip(index + 1) {
            let overlaps = left.x < right.x + right.width
                && right.x < left.x + left.width
                && left.y < right.y + right.height
                && right.y < left.y + left.height;
            ensure!(!overlaps, "mode-selection state artwork footprints overlap");
        }
    }
    ensure!(
        mask.iter().enumerate().all(|(pixel, selected)| {
            if !selected {
                return true;
            }
            let x = u32::try_from(pixel % width).expect("surface x fits u32");
            let y = u32::try_from(pixel / width).expect("surface y fits u32");
            footprints.iter().any(|bounds| {
                x >= bounds.x
                    && x < bounds.x + bounds.width
                    && y >= bounds.y
                    && y < bounds.y + bounds.height
            })
        }),
        "mode-selection source mask escapes its three exact state footprints"
    );
    for bounds in footprints {
        let selected = mask
            .iter()
            .enumerate()
            .filter_map(|(pixel, selected)| {
                if !selected {
                    return None;
                }
                let x = u32::try_from(pixel % width).ok()?;
                let y = u32::try_from(pixel / width).ok()?;
                (x >= bounds.x
                    && x < bounds.x + bounds.width
                    && y >= bounds.y
                    && y < bounds.y + bounds.height)
                    .then_some((x, y))
            })
            .collect::<Vec<_>>();
        let min_x = selected.iter().map(|(x, _)| *x).min();
        let max_x = selected.iter().map(|(x, _)| *x).max();
        let min_y = selected.iter().map(|(_, y)| *y).min();
        let max_y = selected.iter().map(|(_, y)| *y).max();
        ensure!(
            min_x == Some(bounds.x)
                && max_x == Some(bounds.x + bounds.width - 1)
                && min_y == Some(bounds.y)
                && max_y == Some(bounds.y + bounds.height - 1),
            "mode-selection state footprint is not the exact source-mask bounding box"
        );
    }
    Ok(())
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_menu_panel_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    native_placement: PixelBounds,
    consumer: &StylizedArtworkMenuPanelConsumer,
    region: &super::GraphicsTextRegion,
) -> Result<StylizedArtworkMenuPanelConsumerValidation> {
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
            .is_some_and(|record| record.file_record == consumer.load_route.image_file_record)
            && palettes
                .get(consumer.load_route.palette_slot)
                .is_some_and(|record| {
                    record.file_record == consumer.load_route.palette_file_record
                }),
        "menu-panel artwork load-table route changed"
    );
    ensure!(
        consumer.registration_route.load_table_symbol == consumer.load_route.load_table_symbol
            && consumer.registration_route.texture_slot == consumer.pattern_texture_slot
            && consumer.registration_route.image_load_index == consumer.load_route.image_load_index
            && consumer.registration_route.palette_slot == consumer.load_route.palette_slot,
        "menu-panel registration route no longer maps the pattern texture slot"
    );
    let (_, registration_bytes) =
        read_symbol_bytes(file, &consumer.registration_route.registration_set_symbol)?;
    let pairs = parse_texture_registration_pairs(
        &consumer.registration_route.registration_set_symbol,
        &registration_bytes,
    )?;
    ensure!(
        pairs
            .get(consumer.registration_route.texture_slot)
            .is_some_and(|pair| {
                pair.image_load_index == consumer.registration_route.image_load_index
                    && pair.palette_slot == consumer.registration_route.palette_slot
            }),
        "menu-panel registration pair changed"
    );

    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let pattern = patterns
        .get(consumer.pattern_record_index)
        .context("menu-panel pattern record is out of range")?;
    ensure!(
        pattern.image_load_index == consumer.pattern_texture_slot
            && pattern.source_x == consumer.pattern_source_bounds.x
            && pattern.source_y == consumer.pattern_source_bounds.y
            && pattern.width == consumer.pattern_source_bounds.width
            && pattern.height == consumer.pattern_source_bounds.height,
        "menu-panel pattern record changed"
    );
    ensure_bounds_contains(
        consumer.pattern_source_bounds,
        region.bounds,
        "menu-panel Japanese region exceeds its pattern source rectangle",
    )?;

    let expected_screen_x = i32::from(consumer.screen_first_x)
        + i32::from(consumer.screen_horizontal_spacing) * i32::from(consumer.column_index);
    ensure!(
        consumer.screen_horizontal_spacing == 160
            && i16::try_from(expected_screen_x)? == consumer.pattern_screen_origin.x
            && consumer.screen_y == consumer.pattern_screen_origin.y,
        "menu-panel screen grid changed"
    );
    let logical_bounds = derive_screen_bounds(
        consumer.pattern_screen_origin,
        consumer.pattern_source_bounds,
        region.bounds,
    )?;
    ensure!(
        logical_bounds == consumer.derived_logical_screen_bounds,
        "menu-panel logical screen bounds changed"
    );
    let derived_native_screen_bounds = derive_screen_bounds(
        consumer.pattern_screen_origin,
        consumer.pattern_source_bounds,
        native_placement,
    )?;

    ensure!(
        disassemble_symbol(executable, &consumer.actor_symbol)?
            .instructions
            .len()
            == consumer.actor_instruction_count,
        "menu-panel actor instruction population changed"
    );
    let (actor_address, actor_bytes) = read_symbol_bytes(file, &consumer.actor_symbol)?;
    let mut typed_actor_instructions = validate_loaded_address(
        &actor_bytes,
        actor_address,
        0x34,
        0x38,
        GeneralRegister::V0,
        symbol_address(file, &consumer.pattern_table_symbol)?,
        "menu-panel pattern table",
    )?;
    typed_actor_instructions.extend([
        require_memory(
            &actor_bytes,
            actor_address,
            0x30,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::S0,
            consumer.selector_state_offset,
            "menu-panel pattern state",
        )?,
        require_shift(
            &actor_bytes,
            actor_address,
            0x3c,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::A1,
            1,
            "menu-panel pattern index times two",
        )?,
        require_three_general(
            &actor_bytes,
            actor_address,
            0x40,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            GeneralRegister::A1,
            "menu-panel pattern index times three",
        )?,
        require_shift(
            &actor_bytes,
            actor_address,
            0x44,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            3,
            "menu-panel pattern record byte stride",
        )?,
        validate_jump_target(
            &actor_bytes,
            actor_address,
            0x48,
            symbol_address(file, &consumer.pattern_setter_symbol)?,
            "menu-panel pattern setter",
        )?,
        require_three_general(
            &actor_bytes,
            actor_address,
            0x4c,
            InstructionForm::Addu,
            GeneralRegister::A1,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "menu-panel selected pattern address",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            0x54,
            InstructionForm::Addiu,
            GeneralRegister::A1,
            GeneralRegister::ZERO,
            consumer.screen_y,
            "menu-panel screen y position",
        )?,
        require_memory(
            &actor_bytes,
            actor_address,
            0x68,
            InstructionForm::Lw,
            GeneralRegister::A3,
            GeneralRegister::S0,
            consumer.column_state_offset,
            "menu-panel column state",
        )?,
        require_shift(
            &actor_bytes,
            actor_address,
            0x70,
            InstructionForm::Sll,
            GeneralRegister::A2,
            GeneralRegister::A3,
            2,
            "menu-panel column times four",
        )?,
        require_three_general(
            &actor_bytes,
            actor_address,
            0x74,
            InstructionForm::Addu,
            GeneralRegister::A2,
            GeneralRegister::A2,
            GeneralRegister::A3,
            "menu-panel column times five",
        )?,
        require_shift(
            &actor_bytes,
            actor_address,
            0x78,
            InstructionForm::Sll,
            GeneralRegister::A2,
            GeneralRegister::A2,
            5,
            "menu-panel horizontal spacing",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            0x7c,
            InstructionForm::Addiu,
            GeneralRegister::A2,
            GeneralRegister::A2,
            consumer.screen_first_x,
            "menu-panel first screen x position",
        )?,
        require_memory(
            &actor_bytes,
            actor_address,
            0x80,
            InstructionForm::Sw,
            GeneralRegister::A2,
            GeneralRegister::V1,
            0,
            "menu-panel screen x store",
        )?,
        require_memory(
            &actor_bytes,
            actor_address,
            0x88,
            InstructionForm::Sw,
            GeneralRegister::A1,
            GeneralRegister::V1,
            4,
            "menu-panel screen y store",
        )?,
    ]);
    typed_actor_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkMenuPanelConsumerValidation {
        load_route: consumer.load_route.clone(),
        registration_route: consumer.registration_route.clone(),
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        pattern_record_index: consumer.pattern_record_index,
        pattern_texture_slot: consumer.pattern_texture_slot,
        pattern_source_bounds: consumer.pattern_source_bounds,
        actor_symbol: consumer.actor_symbol.clone(),
        selector_state_offset: consumer.selector_state_offset,
        column_state_offset: consumer.column_state_offset,
        column_index: consumer.column_index,
        pattern_screen_origin: consumer.pattern_screen_origin,
        derived_logical_screen_bounds: logical_bounds,
        derived_native_screen_bounds,
        typed_actor_instructions,
        runtime_status: "not observed for this exact artwork overlay",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_scrolling_tile_grid(
    consumer: &StylizedArtworkScrollingTileConsumer,
) -> Result<()> {
    let x_positions = consumer
        .positions
        .iter()
        .map(|position| position.x)
        .collect::<BTreeSet<_>>();
    let y_positions = consumer
        .positions
        .iter()
        .map(|position| position.y)
        .collect::<BTreeSet<_>>();
    let unique_positions = consumer
        .positions
        .iter()
        .map(|position| (position.x, position.y))
        .collect::<BTreeSet<_>>();
    ensure!(
        x_positions.len() >= 2
            && y_positions.len() >= 2
            && unique_positions.len() == consumer.positions.len()
            && x_positions.len() * y_positions.len() == consumer.positions.len(),
        "scrolling stylized artwork positions are not a complete rectangular grid"
    );
    let expected_x_step = i32::try_from(consumer.pattern_source_bounds.width)?;
    let expected_y_step = i32::try_from(consumer.pattern_source_bounds.height)?;
    ensure!(
        x_positions
            .iter()
            .copied()
            .map(i32::from)
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[1] - pair[0] == expected_x_step)
            && y_positions
                .iter()
                .copied()
                .map(i32::from)
                .collect::<Vec<_>>()
                .windows(2)
                .all(|pair| pair[1] - pair[0] == expected_y_step)
            && x_positions.iter().all(|x| {
                y_positions
                    .iter()
                    .all(|y| unique_positions.contains(&(*x, *y)))
            }),
        "scrolling stylized artwork position grid no longer follows the source tile dimensions"
    );
    ensure!(
        consumer.horizontal_wrap_distance
            == consumer.pattern_source_bounds.width * u32::try_from(x_positions.len())?
            && consumer.vertical_wrap_distance
                == consumer.pattern_source_bounds.height * u32::try_from(y_positions.len())?,
        "scrolling stylized artwork wrap distances no longer cover one position grid"
    );
    Ok(())
}
