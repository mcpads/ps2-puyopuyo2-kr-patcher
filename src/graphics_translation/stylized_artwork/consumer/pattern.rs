use super::*;

pub(in crate::graphics_translation::stylized_artwork) fn validate_direct_pattern_display_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    native_placement: PixelBounds,
    consumer: &StylizedArtworkDirectPatternDisplayConsumer,
    region: &super::GraphicsTextRegion,
) -> Result<StylizedArtworkDirectPatternDisplayConsumerValidation> {
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
        "direct-pattern artwork load-table route changed"
    );

    if let Some(registration_route) = &consumer.registration_route {
        ensure!(
            registration_route.load_table_symbol == consumer.load_route.load_table_symbol
                && registration_route.texture_slot == consumer.pattern_texture_slot
                && registration_route.image_load_index == consumer.load_route.image_load_index
                && registration_route.palette_slot == consumer.load_route.palette_slot,
            "direct-pattern registration route no longer maps the pattern texture slot"
        );
        let (_, registration_bytes) =
            read_symbol_bytes(file, &registration_route.registration_set_symbol)?;
        let pairs = parse_texture_registration_pairs(
            &registration_route.registration_set_symbol,
            &registration_bytes,
        )?;
        ensure!(
            pairs
                .get(registration_route.texture_slot)
                .is_some_and(|pair| {
                    pair.image_load_index == registration_route.image_load_index
                        && pair.palette_slot == registration_route.palette_slot
                }),
            "direct-pattern registration pair changed"
        );
    } else {
        ensure!(
            consumer.pattern_texture_slot == consumer.load_route.image_load_index,
            "direct-pattern artwork needs a registration route when its texture slot differs from its image load index"
        );
    }

    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let pattern = patterns
        .get(consumer.pattern_record_index)
        .context("direct-pattern artwork pattern record is out of range")?;
    ensure!(
        pattern.image_load_index == consumer.pattern_texture_slot
            && pattern.source_x == consumer.pattern_source_bounds.x
            && pattern.source_y == consumer.pattern_source_bounds.y
            && pattern.width == consumer.pattern_source_bounds.width
            && pattern.height == consumer.pattern_source_bounds.height,
        "direct-pattern artwork pattern record changed"
    );
    ensure_bounds_contains(
        consumer.pattern_source_bounds,
        region.bounds,
        "direct-pattern Japanese region exceeds its pattern source rectangle",
    )?;

    let logical_bounds = derive_screen_bounds(
        consumer.pattern_screen_origin,
        consumer.pattern_source_bounds,
        region.bounds,
    )?;
    ensure!(
        logical_bounds == consumer.derived_logical_screen_bounds,
        "direct-pattern logical screen bounds changed"
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
        "direct-pattern actor instruction population changed"
    );
    let (actor_address, actor_bytes) = read_symbol_bytes(file, &consumer.actor_symbol)?;
    let pattern_record_offset = consumer
        .pattern_record_index
        .checked_mul(24)
        .context("direct-pattern record offset overflow")?;
    let pattern_record_address = symbol_address(file, &consumer.pattern_table_symbol)?
        .checked_add(u64::try_from(pattern_record_offset)?)
        .context("direct-pattern record address overflow")?;
    let mut typed_actor_instructions = validate_loaded_address(
        &actor_bytes,
        actor_address,
        consumer.pattern_address_upper_offset,
        consumer.pattern_address_lower_offset,
        GeneralRegister::A3,
        pattern_record_address,
        "direct-pattern record address",
    )?;
    typed_actor_instructions.extend([
        require_screen_coordinate(
            &actor_bytes,
            actor_address,
            consumer.screen_x_instruction_offset,
            GeneralRegister::A0,
            consumer.pattern_screen_origin.x,
            "direct-pattern screen x position",
        )?,
        require_screen_coordinate(
            &actor_bytes,
            actor_address,
            consumer.screen_y_instruction_offset,
            GeneralRegister::A1,
            consumer.pattern_screen_origin.y,
            "direct-pattern screen y position",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            consumer.depth_instruction_offset,
            InstructionForm::Addiu,
            GeneralRegister::A2,
            GeneralRegister::ZERO,
            consumer.pattern_depth,
            "direct-pattern display depth",
        )?,
        validate_jump_target(
            &actor_bytes,
            actor_address,
            consumer.display_call_offset,
            symbol_address(file, &consumer.display_constructor_symbol)?,
            "direct-pattern display constructor",
        )?,
    ]);

    let mut protected_pattern_indices = BTreeSet::new();
    for protected_use in &consumer.protected_pattern_uses {
        ensure!(
            protected_use.pattern_record_index != consumer.pattern_record_index
                && protected_pattern_indices.insert(protected_use.pattern_record_index),
            "direct-pattern protected companion records are duplicated or overlap the translated record"
        );
        let protected_pattern = patterns
            .get(protected_use.pattern_record_index)
            .context("direct-pattern protected companion record is out of range")?;
        ensure!(
            protected_use.pattern_texture_slot == consumer.pattern_texture_slot
                && protected_pattern.image_load_index == protected_use.pattern_texture_slot
                && protected_pattern.source_x == protected_use.pattern_source_bounds.x
                && protected_pattern.source_y == protected_use.pattern_source_bounds.y
                && protected_pattern.width == protected_use.pattern_source_bounds.width
                && protected_pattern.height == protected_use.pattern_source_bounds.height,
            "direct-pattern protected companion record changed"
        );
        let protected_record_offset = protected_use
            .pattern_record_index
            .checked_mul(24)
            .context("direct-pattern protected companion offset overflow")?;
        let protected_record_address = symbol_address(file, &consumer.pattern_table_symbol)?
            .checked_add(u64::try_from(protected_record_offset)?)
            .context("direct-pattern protected companion address overflow")?;
        typed_actor_instructions.extend(validate_loaded_address(
            &actor_bytes,
            actor_address,
            protected_use.pattern_address_upper_offset,
            protected_use.pattern_address_lower_offset,
            GeneralRegister::A1,
            protected_record_address,
            "direct-pattern protected companion record address",
        )?);
        typed_actor_instructions.push(validate_jump_target(
            &actor_bytes,
            actor_address,
            protected_use.pattern_setter_call_offset,
            symbol_address(file, &protected_use.pattern_setter_symbol)?,
            "direct-pattern protected companion setter",
        )?);
    }
    typed_actor_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkDirectPatternDisplayConsumerValidation {
        load_route: consumer.load_route.clone(),
        registration_route: consumer.registration_route.clone(),
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        pattern_record_index: consumer.pattern_record_index,
        pattern_texture_slot: consumer.pattern_texture_slot,
        pattern_source_bounds: consumer.pattern_source_bounds,
        actor_symbol: consumer.actor_symbol.clone(),
        display_constructor_symbol: consumer.display_constructor_symbol.clone(),
        pattern_screen_origin: consumer.pattern_screen_origin,
        pattern_depth: consumer.pattern_depth,
        derived_logical_screen_bounds: logical_bounds,
        derived_native_screen_bounds,
        protected_pattern_uses: consumer.protected_pattern_uses.clone(),
        typed_actor_instructions,
        runtime_status: "not observed for this exact artwork overlay",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_indexed_pattern_display_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    native_placement: PixelBounds,
    consumer: &StylizedArtworkIndexedPatternDisplayConsumer,
    region: &super::GraphicsTextRegion,
) -> Result<StylizedArtworkIndexedPatternDisplayConsumerValidation> {
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
        "indexed-pattern artwork load-table route changed"
    );

    ensure!(
        consumer.registration_route.load_table_symbol == consumer.load_route.load_table_symbol
            && consumer.registration_route.texture_slot == consumer.pattern_texture_slot
            && consumer.registration_route.image_load_index == consumer.load_route.image_load_index
            && consumer.registration_route.palette_slot == consumer.load_route.palette_slot,
        "indexed-pattern registration route no longer maps the pattern texture slot"
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
        "indexed-pattern registration pair changed"
    );

    ensure!(
        consumer.selector_value < 2
            && consumer
                .base_pattern_record_index
                .checked_add(usize::from(consumer.selector_value))
                == Some(consumer.pattern_record_index),
        "indexed-pattern selector no longer maps zero and one to consecutive pattern records"
    );
    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let pattern = patterns
        .get(consumer.pattern_record_index)
        .context("indexed-pattern artwork pattern record is out of range")?;
    ensure!(
        pattern.image_load_index == consumer.pattern_texture_slot
            && pattern.source_x == consumer.pattern_source_bounds.x
            && pattern.source_y == consumer.pattern_source_bounds.y
            && pattern.width == consumer.pattern_source_bounds.width
            && pattern.height == consumer.pattern_source_bounds.height,
        "indexed-pattern artwork pattern record changed"
    );
    ensure_bounds_contains(
        consumer.pattern_source_bounds,
        region.bounds,
        "indexed-pattern Japanese region exceeds its pattern source rectangle",
    )?;

    let logical_bounds = derive_screen_bounds(
        consumer.pattern_screen_origin,
        consumer.pattern_source_bounds,
        region.bounds,
    )?;
    ensure!(
        logical_bounds == consumer.derived_logical_screen_bounds,
        "indexed-pattern logical screen bounds changed"
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
        "indexed-pattern actor instruction population changed"
    );
    let (actor_address, actor_bytes) = read_symbol_bytes(file, &consumer.actor_symbol)?;
    ensure!(
        consumer.selector_zero_branch_offset.checked_add(8)
            == Some(consumer.selector_one_value_offset)
            && consumer.selector_one_value_offset.checked_add(4)
                == Some(consumer.selector_other_branch_offset)
            && consumer.selector_other_branch_offset.checked_add(8)
                == Some(consumer.selector_accepted_target_offset)
            && consumer.selector_other_target_offset > consumer.selector_accepted_target_offset
            && consumer.selector_other_target_offset < actor_bytes.len(),
        "indexed-pattern selector guard layout changed"
    );
    let selector_accepted_target = actor_address
        .checked_add(u64::try_from(consumer.selector_accepted_target_offset)?)
        .context("indexed-pattern accepted selector target overflow")?;
    let selector_other_target = actor_address
        .checked_add(u64::try_from(consumer.selector_other_target_offset)?)
        .context("indexed-pattern other selector target overflow")?;

    let mut typed_actor_instructions = validate_loaded_memory_address(
        &actor_bytes,
        actor_address,
        consumer.selector_guard_address_upper_offset,
        consumer.selector_guard_load_offset,
        GeneralRegister::AT,
        InstructionForm::Lhu,
        GeneralRegister::A0,
        consumer.selector_address,
        "indexed-pattern selector guard",
    )?;
    typed_actor_instructions.extend([
        require_branch_two(
            &actor_bytes,
            actor_address,
            consumer.selector_zero_branch_offset,
            InstructionForm::Beq,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            selector_accepted_target,
            "indexed-pattern zero selector branch",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            consumer.selector_one_value_offset,
            InstructionForm::Addiu,
            GeneralRegister::V1,
            GeneralRegister::ZERO,
            1,
            "indexed-pattern one selector value",
        )?,
        require_branch_two(
            &actor_bytes,
            actor_address,
            consumer.selector_other_branch_offset,
            InstructionForm::Bne,
            GeneralRegister::A0,
            GeneralRegister::V1,
            selector_other_target,
            "indexed-pattern other selector branch",
        )?,
    ]);
    typed_actor_instructions.extend(validate_loaded_memory_address(
        &actor_bytes,
        actor_address,
        consumer.selector_load_address_upper_offset,
        consumer.selector_load_offset,
        GeneralRegister::AT,
        InstructionForm::Lhu,
        GeneralRegister::V1,
        consumer.selector_address,
        "indexed-pattern display selector",
    )?);
    typed_actor_instructions.extend(validate_loaded_address(
        &actor_bytes,
        actor_address,
        consumer.pattern_table_address_upper_offset,
        consumer.pattern_table_address_lower_offset,
        GeneralRegister::V0,
        symbol_address(file, &consumer.pattern_table_symbol)?,
        "indexed-pattern table address",
    )?);
    typed_actor_instructions.extend([
        require_screen_coordinate(
            &actor_bytes,
            actor_address,
            consumer.screen_x_instruction_offset,
            GeneralRegister::A0,
            consumer.pattern_screen_origin.x,
            "indexed-pattern screen x position",
        )?,
        require_screen_coordinate(
            &actor_bytes,
            actor_address,
            consumer.screen_y_instruction_offset,
            GeneralRegister::A1,
            consumer.pattern_screen_origin.y,
            "indexed-pattern screen y position",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            consumer.depth_instruction_offset,
            InstructionForm::Addiu,
            GeneralRegister::A2,
            GeneralRegister::ZERO,
            consumer.pattern_depth,
            "indexed-pattern display depth",
        )?,
        require_signed_immediate(
            &actor_bytes,
            actor_address,
            consumer.pattern_index_instruction_offset,
            InstructionForm::Addiu,
            GeneralRegister::A3,
            GeneralRegister::V1,
            i16::try_from(consumer.base_pattern_record_index)?,
            "indexed-pattern base record index",
        )?,
        require_shift(
            &actor_bytes,
            actor_address,
            consumer.pattern_stride_first_shift_offset,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::A3,
            1,
            "indexed-pattern stride first shift",
        )?,
        require_three_general(
            &actor_bytes,
            actor_address,
            consumer.pattern_stride_add_offset,
            InstructionForm::Addu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            GeneralRegister::A3,
            "indexed-pattern stride add",
        )?,
        require_shift(
            &actor_bytes,
            actor_address,
            consumer.pattern_stride_second_shift_offset,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            3,
            "indexed-pattern stride second shift",
        )?,
        validate_jump_target(
            &actor_bytes,
            actor_address,
            consumer.display_call_offset,
            symbol_address(file, &consumer.display_constructor_symbol)?,
            "indexed-pattern display constructor",
        )?,
        require_three_general(
            &actor_bytes,
            actor_address,
            consumer.pattern_address_add_offset,
            InstructionForm::Addu,
            GeneralRegister::A3,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "indexed-pattern selected record address",
        )?,
    ]);
    typed_actor_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(StylizedArtworkIndexedPatternDisplayConsumerValidation {
        load_route: consumer.load_route.clone(),
        registration_route: consumer.registration_route.clone(),
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        base_pattern_record_index: consumer.base_pattern_record_index,
        selector_address: format!("{:#010x}", consumer.selector_address),
        selector_value: consumer.selector_value,
        pattern_record_index: consumer.pattern_record_index,
        pattern_texture_slot: consumer.pattern_texture_slot,
        pattern_source_bounds: consumer.pattern_source_bounds,
        actor_symbol: consumer.actor_symbol.clone(),
        display_constructor_symbol: consumer.display_constructor_symbol.clone(),
        pattern_screen_origin: consumer.pattern_screen_origin,
        pattern_depth: consumer.pattern_depth,
        derived_logical_screen_bounds: logical_bounds,
        derived_native_screen_bounds,
        typed_actor_instructions,
        runtime_status: "not observed for this exact artwork overlay",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn require_screen_coordinate(
    source: &[u8],
    start_address: u64,
    offset: usize,
    destination: GeneralRegister,
    value: i16,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    if value == 0 {
        require_three_general(
            source,
            start_address,
            offset,
            InstructionForm::Daddu,
            destination,
            GeneralRegister::ZERO,
            GeneralRegister::ZERO,
            role,
        )
    } else {
        require_signed_immediate(
            source,
            start_address,
            offset,
            InstructionForm::Addiu,
            destination,
            GeneralRegister::ZERO,
            value,
            role,
        )
    }
}
