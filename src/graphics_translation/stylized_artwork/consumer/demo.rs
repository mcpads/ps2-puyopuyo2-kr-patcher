use super::*;

pub(in crate::graphics_translation::stylized_artwork) fn validate_sequence_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    native_placement: PixelBounds,
    consumer: &StylizedArtworkSequenceConsumer,
    region: &super::GraphicsTextRegion,
    demo_registration_route: Option<&StylizedArtworkDemoRegistrationRoute>,
    legacy_demo_registration_route: Option<&StylizedArtworkLegacyDemoRegistrationRoute>,
) -> Result<StylizedArtworkSequenceConsumerValidation> {
    let (_, load_bytes) = read_symbol_bytes(file, &consumer.load_table_symbol)?;
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
        images.len() == palettes.len()
            && images
                .get(consumer.image_load_index)
                .is_some_and(|record| record.file_record == consumer.image_file_record)
            && palettes
                .get(consumer.palette_slot)
                .is_some_and(|record| record.file_record == consumer.palette_file_record),
        "stylized artwork load-table image or palette route changed"
    );

    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let pattern = patterns
        .get(consumer.pattern_record_index)
        .context("stylized artwork pattern record is out of range")?;
    let pattern_texture_slot = consumer
        .pattern_texture_slot
        .unwrap_or(consumer.image_load_index);
    ensure!(
        pattern.image_load_index == pattern_texture_slot
            && pattern.source_x == consumer.pattern_source_bounds.x
            && pattern.source_y == consumer.pattern_source_bounds.y
            && pattern.width == consumer.pattern_source_bounds.width
            && pattern.height == consumer.pattern_source_bounds.height,
        "stylized artwork pattern record changed"
    );
    match (demo_registration_route, legacy_demo_registration_route) {
        (Some(route), None) => ensure!(
            route.selected_load_table_symbol == consumer.load_table_symbol
                && route.image_load_index == consumer.image_load_index
                && route.palette_slot == consumer.palette_slot
                && route.texture_slot == pattern_texture_slot,
            "stylized artwork sequence consumer disagrees with its demo registration route"
        ),
        (None, Some(route)) => ensure!(
            route.selected_load_table_symbol == consumer.load_table_symbol
                && route.selected_pattern_table_symbol == consumer.pattern_table_symbol
                && route.selected_sequence_symbol == consumer.sequence_symbol
                && route.image_load_index == consumer.image_load_index
                && route.palette_slot == consumer.palette_slot
                && route.texture_slot == pattern_texture_slot,
            "stylized artwork sequence consumer disagrees with its legacy demo registration route"
        ),
        (None, None) => ensure!(
            pattern_texture_slot == consumer.image_load_index,
            "stylized artwork sequence pattern texture slot needs a shared registration route"
        ),
        (Some(_), Some(_)) => {
            anyhow::bail!("stylized artwork sequence consumer has two demo registration routes")
        }
    }
    ensure_bounds_contains(
        consumer.pattern_source_bounds,
        region.bounds,
        "stylized artwork Japanese region exceeds its pattern source rectangle",
    )?;

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
        "stylized artwork sequence opcode size changed"
    );
    if let StylizedArtworkSequenceOperation::SetPattern {
        create_opcode,
        create_command_words,
        ..
    } = &consumer.sequence_operation
    {
        ensure!(
            command_sizes
                .get(*create_opcode)
                .is_some_and(|words| *words == *create_command_words),
            "stylized artwork actor-creation opcode size changed"
        );
    }
    let (_, sequence_bytes) = read_symbol_bytes(file, &consumer.sequence_symbol)?;
    let command = find_sequence_command(
        &consumer.sequence_symbol,
        &sequence_bytes,
        &command_sizes,
        consumer.sequence_command_offset,
    )?;
    let command_matches = match &consumer.sequence_operation {
        StylizedArtworkSequenceOperation::UpdateActor => {
            command.len() == 12
                && i16::from_le_bytes(command[4..6].try_into()?) == consumer.pattern_screen_origin.x
                && i16::from_le_bytes(command[6..8].try_into()?) == consumer.pattern_screen_origin.y
                && u16::from_le_bytes(command[8..10].try_into()?) == consumer.pattern_depth
                && usize::from(u16::from_le_bytes(command[10..12].try_into()?))
                    == consumer.pattern_record_index
        }
        StylizedArtworkSequenceOperation::SetPattern {
            create_command_offset,
            create_opcode,
            create_flags,
            initial_pattern_record_index,
            ..
        } => {
            ensure!(
                create_command_offset < &consumer.sequence_command_offset,
                "pattern-selection command precedes its actor creation"
            );
            let create_command = find_sequence_command(
                &consumer.sequence_symbol,
                &sequence_bytes,
                &command_sizes,
                *create_command_offset,
            )?;
            ensure!(
                create_command.len() == 14
                    && usize::from(u16::from_le_bytes(create_command[0..2].try_into()?))
                        == *create_opcode
                    && u16::from_le_bytes(create_command[2..4].try_into()?)
                        == consumer.sequence_actor_index
                    && u16::from_le_bytes(create_command[4..6].try_into()?) == *create_flags
                    && i16::from_le_bytes(create_command[6..8].try_into()?)
                        == consumer.pattern_screen_origin.x
                    && i16::from_le_bytes(create_command[8..10].try_into()?)
                        == consumer.pattern_screen_origin.y
                    && u16::from_le_bytes(create_command[10..12].try_into()?)
                        == consumer.pattern_depth
                    && usize::from(u16::from_le_bytes(create_command[12..14].try_into()?))
                        == *initial_pattern_record_index,
                "pattern-selection actor-creation command changed"
            );
            command.len() == 6
                && usize::from(u16::from_le_bytes(command[4..6].try_into()?))
                    == consumer.pattern_record_index
        }
        StylizedArtworkSequenceOperation::CreateActor { flags } => {
            command.len() == 14
                && u16::from_le_bytes(command[4..6].try_into()?) == *flags
                && i16::from_le_bytes(command[6..8].try_into()?) == consumer.pattern_screen_origin.x
                && i16::from_le_bytes(command[8..10].try_into()?)
                    == consumer.pattern_screen_origin.y
                && u16::from_le_bytes(command[10..12].try_into()?) == consumer.pattern_depth
                && usize::from(u16::from_le_bytes(command[12..14].try_into()?))
                    == consumer.pattern_record_index
        }
        StylizedArtworkSequenceOperation::CreateChildActor {
            flags,
            parent_actor_index,
            parent_create_command_offset,
            parent_create_opcode,
            parent_create_command_words,
            ..
        } => {
            ensure!(
                parent_create_command_offset < &consumer.sequence_command_offset
                    && command_sizes
                        .get(*parent_create_opcode)
                        .is_some_and(|words| *words == *parent_create_command_words),
                "child actor parent-creation command declaration changed"
            );
            let parent_command = find_sequence_command(
                &consumer.sequence_symbol,
                &sequence_bytes,
                &command_sizes,
                *parent_create_command_offset,
            )?;
            ensure!(
                parent_command.len() == 14
                    && usize::from(u16::from_le_bytes(parent_command[0..2].try_into()?))
                        == *parent_create_opcode
                    && u16::from_le_bytes(parent_command[2..4].try_into()?) == *parent_actor_index,
                "child actor parent-creation command fields changed"
            );

            let parent_x = i16::from_le_bytes(parent_command[6..8].try_into()?);
            let parent_y = i16::from_le_bytes(parent_command[8..10].try_into()?);
            let parent_depth = u16::from_le_bytes(parent_command[10..12].try_into()?);
            let relative_x = i16::from_le_bytes(command[8..10].try_into()?);
            let relative_y = i16::from_le_bytes(command[10..12].try_into()?);
            let relative_depth = i16::from_le_bytes(command[12..14].try_into()?);
            let derived_x = i32::from(parent_x) + i32::from(relative_x);
            let derived_y = i32::from(parent_y) + i32::from(relative_y);
            let derived_depth = i32::from(parent_depth) + i32::from(relative_depth);

            command.len() == 16
                && u16::from_le_bytes(command[4..6].try_into()?) == *flags
                && u16::from_le_bytes(command[6..8].try_into()?) == *parent_actor_index
                && i16::try_from(derived_x).ok() == Some(consumer.pattern_screen_origin.x)
                && i16::try_from(derived_y).ok() == Some(consumer.pattern_screen_origin.y)
                && u16::try_from(derived_depth).ok() == Some(consumer.pattern_depth)
                && usize::from(u16::from_le_bytes(command[14..16].try_into()?))
                    == consumer.pattern_record_index
        }
    };
    ensure!(
        u16::from_le_bytes(command[0..2].try_into()?) as usize == consumer.sequence_opcode
            && u16::from_le_bytes(command[2..4].try_into()?) == consumer.sequence_actor_index
            && command_matches,
        "stylized artwork sequence command fields changed"
    );

    let logical_bounds = derive_screen_bounds(
        consumer.pattern_screen_origin,
        consumer.pattern_source_bounds,
        region.bounds,
    )?;
    ensure!(
        logical_bounds == consumer.derived_logical_screen_bounds,
        "stylized artwork logical screen bounds changed"
    );
    let derived_native_screen_bounds = derive_screen_bounds(
        consumer.pattern_screen_origin,
        consumer.pattern_source_bounds,
        native_placement,
    )?;
    let typed_decoder_instructions = match &consumer.sequence_operation {
        StylizedArtworkSequenceOperation::UpdateActor => {
            validate_update_actor_sequence_decoder(file, executable, consumer)?
        }
        StylizedArtworkSequenceOperation::SetPattern {
            create_opcode,
            create_case_address,
            ..
        } => {
            let mut evidence = validate_set_pattern_sequence_decoder(file, executable, consumer)?;
            evidence.extend(validate_create_actor_pattern_decoder(
                file,
                executable,
                *create_opcode,
                *create_case_address,
                &consumer.pattern_setter_symbol,
            )?);
            evidence.sort_by(|left, right| left.address.cmp(&right.address));
            evidence
        }
        StylizedArtworkSequenceOperation::CreateActor { .. } => {
            validate_create_actor_pattern_decoder(
                file,
                executable,
                consumer.sequence_opcode,
                consumer.sequence_case_address,
                &consumer.pattern_setter_symbol,
            )?
        }
        StylizedArtworkSequenceOperation::CreateChildActor {
            parent_create_opcode,
            parent_create_case_address,
            ..
        } => validate_create_child_actor_pattern_decoder(
            file,
            executable,
            consumer,
            *parent_create_opcode,
            *parent_create_case_address,
        )?,
    };
    let movesys = consumer
        .movesys_route
        .as_ref()
        .map(|route| {
            let demo_route = demo_registration_route
                .context("movesys sequence consumer has no demo registration route")?;
            validate_sequence_movesys_route(file, executable, consumer, demo_route, route)
        })
        .transpose()?;

    Ok(StylizedArtworkSequenceConsumerValidation {
        load_table_symbol: consumer.load_table_symbol.clone(),
        image_load_index: consumer.image_load_index,
        image_file_record: consumer.image_file_record,
        palette_slot: consumer.palette_slot,
        palette_file_record: consumer.palette_file_record,
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        pattern_record_index: consumer.pattern_record_index,
        pattern_texture_slot,
        pattern_source_bounds: consumer.pattern_source_bounds,
        sequence_symbol: consumer.sequence_symbol.clone(),
        sequence_command_offset: consumer.sequence_command_offset,
        sequence_opcode: consumer.sequence_opcode,
        sequence_actor_index: consumer.sequence_actor_index,
        sequence_operation: consumer.sequence_operation.clone(),
        pattern_screen_origin: consumer.pattern_screen_origin,
        pattern_depth: consumer.pattern_depth,
        derived_logical_screen_bounds: logical_bounds,
        derived_native_screen_bounds,
        typed_decoder_instructions,
        movesys,
        runtime_status: "not observed for this exact artwork overlay",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_animated_pattern_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    native_placement: PixelBounds,
    consumer: &StylizedArtworkAnimatedPatternConsumer,
    region: &super::GraphicsTextRegion,
    legacy_demo_registration_route: &StylizedArtworkLegacyDemoRegistrationRoute,
) -> Result<StylizedArtworkAnimatedPatternConsumerValidation> {
    ensure!(
        legacy_demo_registration_route.selected_load_table_symbol == consumer.load_table_symbol
            && legacy_demo_registration_route.selected_pattern_table_symbol
                == consumer.pattern_table_symbol
            && legacy_demo_registration_route.selected_animation_table_symbol
                == consumer.animation_pointer_table_symbol
            && legacy_demo_registration_route.selected_sequence_symbol == consumer.sequence_symbol
            && legacy_demo_registration_route.image_load_index == consumer.image_load_index
            && legacy_demo_registration_route.palette_slot == consumer.palette_slot
            && legacy_demo_registration_route.texture_slot == consumer.pattern_texture_slot,
        "animated pattern consumer disagrees with its legacy demo registration route"
    );

    let (_, load_bytes) = read_symbol_bytes(file, &consumer.load_table_symbol)?;
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
        images.len() == palettes.len()
            && images
                .get(consumer.image_load_index)
                .is_some_and(|record| record.file_record == consumer.image_file_record)
            && palettes
                .get(consumer.palette_slot)
                .is_some_and(|record| record.file_record == consumer.palette_file_record),
        "animated pattern load-table image or palette route changed"
    );

    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let declared_animation_frames = std::iter::once(StylizedArtworkAnimationFrame {
        pattern_record_index: consumer.pattern_record_index,
        pattern_source_bounds: consumer.pattern_source_bounds,
        frame_duration: consumer.animation_frame_duration,
        derived_logical_screen_bounds: consumer.derived_logical_screen_bounds,
    })
    .chain(consumer.subsequent_animation_frames.iter().copied())
    .collect::<Vec<_>>();
    let mut frame_pattern_indices = BTreeSet::new();
    ensure!(
        consumer.animation_terminator >= 0xfffa
            && declared_animation_frames.iter().all(|frame| {
                frame.frame_duration > 0 && frame_pattern_indices.insert(frame.pattern_record_index)
            }),
        "animated pattern frame declaration is empty, duplicated, or has an invalid terminator"
    );
    for frame in &declared_animation_frames {
        let pattern = patterns
            .get(frame.pattern_record_index)
            .context("animated pattern record is out of range")?;
        ensure!(
            pattern.image_load_index == consumer.pattern_texture_slot
                && pattern.source_x == frame.pattern_source_bounds.x
                && pattern.source_y == frame.pattern_source_bounds.y
                && pattern.width == frame.pattern_source_bounds.width
                && pattern.height == frame.pattern_source_bounds.height,
            "animated pattern frame record changed"
        );
        ensure_bounds_contains(
            frame.pattern_source_bounds,
            region.bounds,
            "animated pattern Japanese region exceeds a frame source rectangle",
        )?;
    }
    ensure!(
        patterns
            .get(consumer.initial_pattern_record_index)
            .is_some_and(|initial| initial.image_load_index == consumer.pattern_texture_slot),
        "animated pattern initial actor pattern changed"
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
            .get(consumer.create_opcode)
            .is_some_and(|words| *words == consumer.create_command_words)
            && command_sizes
                .get(consumer.animation_opcode)
                .is_some_and(|words| *words == consumer.animation_command_words),
        "animated pattern sequence command size changed"
    );
    let (_, sequence_bytes) = read_symbol_bytes(file, &consumer.sequence_symbol)?;
    let create_command = find_sequence_command(
        &consumer.sequence_symbol,
        &sequence_bytes,
        &command_sizes,
        consumer.create_command_offset,
    )?;
    ensure!(
        create_command.len() == 14
            && usize::from(u16::from_le_bytes(create_command[0..2].try_into()?))
                == consumer.create_opcode
            && u16::from_le_bytes(create_command[2..4].try_into()?)
                == consumer.sequence_actor_index
            && u16::from_le_bytes(create_command[4..6].try_into()?) == consumer.create_flags
            && i16::from_le_bytes(create_command[6..8].try_into()?)
                == consumer.pattern_screen_origin.x
            && i16::from_le_bytes(create_command[8..10].try_into()?)
                == consumer.pattern_screen_origin.y
            && u16::from_le_bytes(create_command[10..12].try_into()?) == consumer.pattern_depth
            && usize::from(u16::from_le_bytes(create_command[12..14].try_into()?))
                == consumer.initial_pattern_record_index,
        "animated pattern actor-creation command changed"
    );
    let animation_command = find_sequence_command(
        &consumer.sequence_symbol,
        &sequence_bytes,
        &command_sizes,
        consumer.animation_command_offset,
    )?;
    ensure!(
        animation_command.len() == 6
            && usize::from(u16::from_le_bytes(animation_command[0..2].try_into()?))
                == consumer.animation_opcode
            && u16::from_le_bytes(animation_command[2..4].try_into()?)
                == consumer.sequence_actor_index
            && usize::from(u16::from_le_bytes(animation_command[4..6].try_into()?))
                == consumer.animation_index,
        "animated pattern animation-selection command changed"
    );

    let animation_pointers =
        read_pointer_table_addresses(file, &consumer.animation_pointer_table_symbol)?;
    ensure!(
        animation_pointers.get(consumer.animation_index).copied()
            == Some(symbol_address(file, &consumer.animation_table_symbol)?),
        "animated pattern animation pointer changed"
    );
    let (_, animation_bytes) = read_symbol_bytes(file, &consumer.animation_table_symbol)?;
    let expected_animation_bytes = declared_animation_frames
        .len()
        .checked_add(1)
        .and_then(|records| records.checked_mul(4))
        .context("animated pattern frame-table length overflow")?;
    ensure!(
        animation_bytes.len() == expected_animation_bytes,
        "animated pattern frame table length changed"
    );
    for (record, frame) in animation_bytes
        .chunks_exact(4)
        .zip(&declared_animation_frames)
    {
        ensure!(
            usize::from(u16::from_le_bytes(record[0..2].try_into()?)) == frame.pattern_record_index
                && u16::from_le_bytes(record[2..4].try_into()?) == frame.frame_duration,
            "animated pattern frame table changed"
        );
    }
    let terminator = animation_bytes
        .get(animation_bytes.len() - 4..)
        .context("animated pattern frame table has no terminator")?;
    ensure!(
        u16::from_le_bytes(terminator[0..2].try_into()?) == consumer.animation_terminator
            && u16::from_le_bytes(terminator[2..4].try_into()?) == 0,
        "animated pattern frame table changed"
    );

    let animation_frames = declared_animation_frames
        .iter()
        .map(|frame| {
            let derived_logical_screen_bounds = derive_screen_bounds(
                consumer.pattern_screen_origin,
                frame.pattern_source_bounds,
                region.bounds,
            )?;
            ensure!(
                derived_logical_screen_bounds == frame.derived_logical_screen_bounds,
                "animated pattern logical screen bounds changed"
            );
            Ok(StylizedArtworkAnimationFrameValidation {
                pattern_record_index: frame.pattern_record_index,
                pattern_source_bounds: frame.pattern_source_bounds,
                frame_duration: frame.frame_duration,
                derived_logical_screen_bounds,
                derived_native_screen_bounds: derive_screen_bounds(
                    consumer.pattern_screen_origin,
                    frame.pattern_source_bounds,
                    native_placement,
                )?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let primary_frame = animation_frames
        .first()
        .context("animated pattern has no primary frame")?;
    let logical_bounds = primary_frame.derived_logical_screen_bounds;
    let derived_native_screen_bounds = primary_frame.derived_native_screen_bounds;
    let typed_create_decoder_instructions = validate_create_actor_pattern_decoder(
        file,
        executable,
        consumer.create_opcode,
        consumer.create_case_address,
        &consumer.pattern_setter_symbol,
    )?;
    let typed_animation_decoder_instructions = validate_animation_sequence_decoder(
        file,
        executable,
        AnimationSequenceDecoderRoute {
            animation_opcode: consumer.animation_opcode,
            animation_case_address: consumer.animation_case_address,
            cell_initializer_symbol: &consumer.cell_initializer_symbol,
            animation_initializer_symbol: &consumer.animation_initializer_symbol,
        },
    )?;

    Ok(StylizedArtworkAnimatedPatternConsumerValidation {
        load_table_symbol: consumer.load_table_symbol.clone(),
        image_load_index: consumer.image_load_index,
        image_file_record: consumer.image_file_record,
        palette_slot: consumer.palette_slot,
        palette_file_record: consumer.palette_file_record,
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        pattern_record_index: consumer.pattern_record_index,
        pattern_texture_slot: consumer.pattern_texture_slot,
        pattern_source_bounds: consumer.pattern_source_bounds,
        sequence_symbol: consumer.sequence_symbol.clone(),
        create_command_offset: consumer.create_command_offset,
        animation_command_offset: consumer.animation_command_offset,
        sequence_actor_index: consumer.sequence_actor_index,
        animation_index: consumer.animation_index,
        animation_pointer_table_symbol: consumer.animation_pointer_table_symbol.clone(),
        animation_table_symbol: consumer.animation_table_symbol.clone(),
        animation_frame_duration: consumer.animation_frame_duration,
        animation_frames,
        animation_terminator: consumer.animation_terminator,
        pattern_screen_origin: consumer.pattern_screen_origin,
        pattern_depth: consumer.pattern_depth,
        derived_logical_screen_bounds: logical_bounds,
        derived_native_screen_bounds,
        typed_create_decoder_instructions,
        typed_animation_decoder_instructions,
        runtime_status: "not observed for this exact artwork overlay",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_sequence_movesys_route(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkSequenceConsumer,
    demo_route: &StylizedArtworkDemoRegistrationRoute,
    route: &StylizedArtworkMovesysRoute,
) -> Result<StylizedArtworkMovesysValidation> {
    ensure!(
        !route.demo_pointer_table_symbol.trim().is_empty()
            && route.demo_index == demo_route.demo_index
            && !route.selected_pointer_table_symbol.trim().is_empty()
            && !route.script_symbol.trim().is_empty()
            && !route.command_size_symbol.trim().is_empty()
            && !route.commands.is_empty()
            && route
                .commands
                .last()
                .is_some_and(|command| { command.opcode == 0 && command.operands.is_empty() })
            && route.transform_opcode == 0x2c
            && route.transform_command_words == 4
            && route.initialize_opcode == 0x25
            && route.initialize_command_words == 3
            && route.transform_command_offset != route.initialize_command_offset
            && route.sequence_creator_symbol == "Seq_Create"
            && route.sequence_creator_instruction_count > 0
            && route.initializer_symbol == "Act_MovesysInit"
            && route.initializer_instruction_count > 0,
        "movesys route header is incomplete or changed"
    );

    let demo_pointers = read_pointer_table_addresses(file, &route.demo_pointer_table_symbol)?;
    ensure!(
        demo_pointers.get(route.demo_index).copied()
            == Some(symbol_address(file, &route.selected_pointer_table_symbol)?),
        "demo movesys pointer-table selection changed"
    );
    let script_pointers = read_pointer_table_addresses(file, &route.selected_pointer_table_symbol)?;
    ensure!(
        script_pointers.get(route.script_index).copied()
            == Some(symbol_address(file, &route.script_symbol)?),
        "selected movesys script pointer changed"
    );

    let (_, sequence_command_size_bytes) = read_symbol_bytes(file, SEQUENCE_COMMAND_SIZE_SYMBOL)?;
    ensure!(
        sequence_command_size_bytes.len().is_multiple_of(2),
        "SeqComSize is not a sequence of 16-bit sizes"
    );
    let sequence_command_sizes = sequence_command_size_bytes
        .chunks_exact(2)
        .map(|bytes| usize::from(u16::from_le_bytes(bytes.try_into().expect("two-byte word"))))
        .collect::<Vec<_>>();
    ensure!(
        sequence_command_sizes
            .get(route.transform_opcode)
            .is_some_and(|words| *words == route.transform_command_words)
            && sequence_command_sizes
                .get(route.initialize_opcode)
                .is_some_and(|words| *words == route.initialize_command_words),
        "movesys sequence command sizes changed"
    );
    let (_, sequence_bytes) = read_symbol_bytes(file, &consumer.sequence_symbol)?;
    let transform_command = find_sequence_command(
        &consumer.sequence_symbol,
        &sequence_bytes,
        &sequence_command_sizes,
        route.transform_command_offset,
    )?;
    ensure!(
        transform_command.len() == 8
            && usize::from(u16::from_le_bytes(transform_command[0..2].try_into()?))
                == route.transform_opcode
            && u16::from_le_bytes(transform_command[2..4].try_into()?)
                == consumer.sequence_actor_index
            && i16::from_le_bytes(transform_command[4..6].try_into()?) == route.transform_anchor.x
            && i16::from_le_bytes(transform_command[6..8].try_into()?) == route.transform_anchor.y,
        "movesys transform-anchor command changed"
    );
    let initialize_command = find_sequence_command(
        &consumer.sequence_symbol,
        &sequence_bytes,
        &sequence_command_sizes,
        route.initialize_command_offset,
    )?;
    ensure!(
        initialize_command.len() == 6
            && usize::from(u16::from_le_bytes(initialize_command[0..2].try_into()?))
                == route.initialize_opcode
            && u16::from_le_bytes(initialize_command[2..4].try_into()?)
                == consumer.sequence_actor_index
            && usize::from(u16::from_le_bytes(initialize_command[4..6].try_into()?))
                == route.script_index,
        "movesys initializer command changed"
    );

    let (_, move_command_sizes) = read_symbol_bytes(file, &route.command_size_symbol)?;
    ensure!(
        !move_command_sizes.is_empty() && move_command_sizes.iter().all(|words| *words > 0),
        "movesys command-size table is empty or contains a zero-sized command"
    );
    let (_, script_bytes) = read_symbol_bytes(file, &route.script_symbol)?;
    ensure!(
        !script_bytes.is_empty() && script_bytes.len().is_multiple_of(2),
        "movesys script is not a nonempty sequence of 16-bit words"
    );
    let mut actual_commands = Vec::new();
    let mut script_offset = 0;
    while script_offset < script_bytes.len() {
        let opcode = usize::from(u16::from_le_bytes(
            script_bytes[script_offset..script_offset + 2].try_into()?,
        ));
        let words = usize::from(
            *move_command_sizes
                .get(opcode)
                .with_context(|| format!("movesys script opcode {opcode:#x} has no size"))?,
        );
        let byte_length = words
            .checked_mul(2)
            .context("movesys command length overflow")?;
        let command = script_bytes
            .get(script_offset..script_offset + byte_length)
            .with_context(|| format!("movesys script ends inside opcode {opcode:#x}"))?;
        actual_commands.push(StylizedArtworkMovesysCommand {
            opcode,
            operands: command[2..]
                .chunks_exact(2)
                .map(|word| i16::from_le_bytes(word.try_into().expect("two-byte word")))
                .collect(),
        });
        script_offset += byte_length;
    }
    ensure!(
        script_offset == script_bytes.len() && actual_commands == route.commands,
        "movesys script command stream changed"
    );

    let (runner_address, runner_bytes) = read_symbol_bytes(file, &demo_route.runner_symbol)?;
    let mut typed_runner_instructions = validate_loaded_address(
        &runner_bytes,
        runner_address,
        0x144,
        0x158,
        GeneralRegister::V0,
        symbol_address(file, &route.demo_pointer_table_symbol)?,
        "demo movesys pointer-table base",
    )?;
    typed_runner_instructions.extend([
        require_shift(
            &runner_bytes,
            runner_address,
            0x160,
            InstructionForm::Sll,
            GeneralRegister::T1,
            GeneralRegister::T0,
            2,
            "demo movesys pointer index",
        )?,
        require_three_general(
            &runner_bytes,
            runner_address,
            0x170,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::T1,
            "demo movesys pointer address",
        )?,
        require_memory(
            &runner_bytes,
            runner_address,
            0x17c,
            InstructionForm::Lw,
            GeneralRegister::T1,
            GeneralRegister::V0,
            0,
            "selected demo movesys pointer",
        )?,
        validate_jump_target(
            &runner_bytes,
            runner_address,
            0x184,
            symbol_address(file, &route.sequence_creator_symbol)?,
            "demo sequence creator",
        )?,
    ]);
    typed_runner_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    ensure!(
        disassemble_symbol(executable, &route.sequence_creator_symbol)?
            .instructions
            .len()
            == route.sequence_creator_instruction_count,
        "sequence creator instruction population changed"
    );
    let (creator_address, creator_bytes) = read_symbol_bytes(file, &route.sequence_creator_symbol)?;
    let typed_sequence_creator_instructions = vec![require_memory(
        &creator_bytes,
        creator_address,
        0x60,
        InstructionForm::Sw,
        GeneralRegister::T1,
        GeneralRegister::V0,
        0x228,
        "sequence movesys pointer storage",
    )?];

    ensure!(
        disassemble_symbol(executable, SEQUENCE_COMMAND_SYMBOL)?
            .instructions
            .len()
            == SEQUENCE_COMMAND_INSTRUCTIONS,
        "SeqCommand instruction population changed"
    );
    let (decoder_address, decoder_bytes) = read_symbol_bytes(file, SEQUENCE_COMMAND_SYMBOL)?;
    let dispatch = read_address_range(
        file,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        u64::try_from(COMMAND_DISPATCH_ENTRIES * 4)?,
    )?;
    let dispatch_case = |opcode: usize| -> Result<usize> {
        ensure!(
            (COMMAND_DISPATCH_FIRST_OPCODE
                ..COMMAND_DISPATCH_FIRST_OPCODE + COMMAND_DISPATCH_ENTRIES)
                .contains(&opcode),
            "movesys opcode is outside the SeqCommand dispatch table"
        );
        let entry_offset = (opcode - COMMAND_DISPATCH_FIRST_OPCODE) * 4;
        let case_address = u64::from(u32::from_le_bytes(
            dispatch[entry_offset..entry_offset + 4]
                .try_into()
                .expect("fixed dispatch entry width"),
        ));
        usize::try_from(
            case_address
                .checked_sub(decoder_address)
                .context("movesys sequence case precedes SeqCommand")?,
        )
        .context("movesys sequence case offset does not fit usize")
    };
    let transform_case = dispatch_case(route.transform_opcode)?;
    let initialize_case = dispatch_case(route.initialize_opcode)?;
    let mut typed_sequence_decoder_instructions = vec![
        require_memory(
            &decoder_bytes,
            decoder_address,
            transform_case,
            InstructionForm::Lhu,
            GeneralRegister::A0,
            GeneralRegister::V1,
            2,
            "movesys transform actor index",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            transform_case + 0x10,
            InstructionForm::Lh,
            GeneralRegister::V1,
            GeneralRegister::V1,
            4,
            "movesys transform anchor x",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            transform_case + 0x1c,
            InstructionForm::Sw,
            GeneralRegister::V1,
            GeneralRegister::A0,
            0xa0,
            "movesys transform anchor x storage",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            transform_case + 0x20,
            InstructionForm::Lw,
            GeneralRegister::V1,
            GeneralRegister::S1,
            0x22c,
            "movesys transform command pointer",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            transform_case + 0x24,
            InstructionForm::Lh,
            GeneralRegister::V1,
            GeneralRegister::V1,
            6,
            "movesys transform anchor y",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            transform_case + 0x28,
            InstructionForm::Sw,
            GeneralRegister::V1,
            GeneralRegister::A0,
            0xa4,
            "movesys transform anchor y storage",
        )?,
        require_unsigned_immediate(
            &decoder_bytes,
            decoder_address,
            transform_case + 0x30,
            InstructionForm::Ori,
            GeneralRegister::V1,
            GeneralRegister::V1,
            8,
            "movesys transform flag",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            initialize_case,
            InstructionForm::Lhu,
            GeneralRegister::A0,
            GeneralRegister::V1,
            2,
            "movesys initializer actor index",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            initialize_case + 4,
            InstructionForm::Lw,
            GeneralRegister::V0,
            GeneralRegister::S1,
            0x228,
            "movesys initializer pointer table",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            initialize_case + 8,
            InstructionForm::Lhu,
            GeneralRegister::V1,
            GeneralRegister::V1,
            4,
            "movesys initializer script index",
        )?,
        require_shift(
            &decoder_bytes,
            decoder_address,
            initialize_case + 0x0c,
            InstructionForm::Sll,
            GeneralRegister::A0,
            GeneralRegister::A0,
            2,
            "movesys initializer actor pointer index",
        )?,
        require_shift(
            &decoder_bytes,
            decoder_address,
            initialize_case + 0x14,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "movesys initializer script pointer index",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            initialize_case + 0x1c,
            InstructionForm::Lw,
            GeneralRegister::A1,
            GeneralRegister::V0,
            0,
            "selected movesys script pointer",
        )?,
        validate_jump_target(
            &decoder_bytes,
            decoder_address,
            initialize_case + 0x20,
            symbol_address(file, &route.initializer_symbol)?,
            "movesys initializer",
        )?,
        require_memory(
            &decoder_bytes,
            decoder_address,
            initialize_case + 0x24,
            InstructionForm::Lw,
            GeneralRegister::A0,
            GeneralRegister::A0,
            0x20,
            "movesys actor pointer",
        )?,
    ];
    typed_sequence_decoder_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    ensure!(
        disassemble_symbol(executable, &route.initializer_symbol)?
            .instructions
            .len()
            == route.initializer_instruction_count,
        "movesys initializer instruction population changed"
    );
    let (initializer_address, initializer_bytes) =
        read_symbol_bytes(file, &route.initializer_symbol)?;
    let typed_initializer_instructions = vec![require_memory(
        &initializer_bytes,
        initializer_address,
        0,
        InstructionForm::Sw,
        GeneralRegister::A1,
        GeneralRegister::A0,
        0x54,
        "movesys script pointer storage",
    )?];

    Ok(StylizedArtworkMovesysValidation {
        route: route.clone(),
        typed_runner_instructions,
        typed_sequence_creator_instructions,
        typed_sequence_decoder_instructions,
        typed_initializer_instructions,
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_source_pattern_consumer(
    file: &object::File<'_>,
    consumer: &StylizedArtworkSourcePatternConsumer,
    region: &super::GraphicsTextRegion,
    demo_registration_route: &StylizedArtworkDemoRegistrationRoute,
) -> Result<StylizedArtworkSourcePatternConsumerValidation> {
    ensure!(
        !consumer.animation_table_symbols.is_empty()
            && consumer.create_pattern_opcode != consumer.update_pattern_opcode
            && consumer
                .animation_table_symbols
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                == consumer.animation_table_symbols.len(),
        "source pattern consumer header is incomplete or duplicated"
    );
    ensure!(
        demo_registration_route.selected_load_table_symbol == consumer.load_table_symbol
            && demo_registration_route.image_load_index == consumer.image_load_index
            && demo_registration_route.palette_slot == consumer.palette_slot
            && demo_registration_route.texture_slot == consumer.pattern_texture_slot,
        "source pattern consumer disagrees with its demo registration route"
    );

    let (_, load_bytes) = read_symbol_bytes(file, &consumer.load_table_symbol)?;
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
        images.len() == palettes.len()
            && images
                .get(consumer.image_load_index)
                .is_some_and(|record| record.file_record == consumer.image_file_record)
            && palettes
                .get(consumer.palette_slot)
                .is_some_and(|record| record.file_record == consumer.palette_file_record),
        "source pattern load-table image or palette route changed"
    );

    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let pattern = patterns
        .get(consumer.pattern_record_index)
        .context("source pattern record is out of range")?;
    ensure!(
        pattern.image_load_index == consumer.pattern_texture_slot
            && pattern.source_x == consumer.pattern_source_bounds.x
            && pattern.source_y == consumer.pattern_source_bounds.y
            && pattern.width == consumer.pattern_source_bounds.width
            && pattern.height == consumer.pattern_source_bounds.height,
        "source pattern record changed"
    );
    ensure_bounds_contains(
        consumer.pattern_source_bounds,
        region.bounds,
        "source pattern Japanese region exceeds its pattern rectangle",
    )?;

    let direct_sequence_pattern_references = collect_sequence_pattern_references(
        file,
        &consumer.sequence_symbol,
        consumer.create_pattern_opcode,
        consumer.update_pattern_opcode,
    )?;
    ensure!(
        !direct_sequence_pattern_references.contains(&consumer.pattern_record_index),
        "source pattern is selected by a declared sequence command"
    );

    let animation_pattern_references = collect_animation_pattern_references(
        file,
        &consumer.animation_pointer_table_symbol,
        &consumer.animation_table_symbols,
    )?;
    ensure!(
        !animation_pattern_references.contains(&consumer.pattern_record_index),
        "source pattern is selected by a declared animation table"
    );

    Ok(StylizedArtworkSourcePatternConsumerValidation {
        load_table_symbol: consumer.load_table_symbol.clone(),
        image_load_index: consumer.image_load_index,
        image_file_record: consumer.image_file_record,
        palette_slot: consumer.palette_slot,
        palette_file_record: consumer.palette_file_record,
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        pattern_record_index: consumer.pattern_record_index,
        pattern_texture_slot: consumer.pattern_texture_slot,
        pattern_source_bounds: consumer.pattern_source_bounds,
        sequence_symbol: consumer.sequence_symbol.clone(),
        animation_pointer_table_symbol: consumer.animation_pointer_table_symbol.clone(),
        direct_sequence_pattern_references,
        animation_pattern_references,
        runtime_status: "no selected sequence or animation consumer in the declared demo route",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_unreferenced_source_consumer(
    file: &object::File<'_>,
    consumer: &StylizedArtworkUnreferencedSourceConsumer,
    region: &super::GraphicsTextRegion,
    legacy_demo_registration_route: &StylizedArtworkLegacyDemoRegistrationRoute,
) -> Result<StylizedArtworkUnreferencedSourceConsumerValidation> {
    ensure!(
        !consumer.animation_table_symbols.is_empty()
            && consumer.create_pattern_opcode != consumer.update_pattern_opcode
            && consumer
                .animation_table_symbols
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                == consumer.animation_table_symbols.len(),
        "unreferenced source consumer header is incomplete or duplicated"
    );
    ensure!(
        legacy_demo_registration_route.selected_load_table_symbol == consumer.load_table_symbol
            && legacy_demo_registration_route.selected_pattern_table_symbol
                == consumer.pattern_table_symbol
            && legacy_demo_registration_route.selected_animation_table_symbol
                == consumer.animation_pointer_table_symbol
            && legacy_demo_registration_route.selected_sequence_symbol == consumer.sequence_symbol
            && legacy_demo_registration_route.image_load_index == consumer.image_load_index
            && legacy_demo_registration_route.palette_slot == consumer.palette_slot
            && legacy_demo_registration_route.texture_slot == consumer.pattern_texture_slot,
        "unreferenced source consumer disagrees with its legacy demo registration route"
    );
    ensure!(
        consumer.source_bounds == region.bounds,
        "unreferenced source bounds differ from the exact Japanese region"
    );

    let (_, load_bytes) = read_symbol_bytes(file, &consumer.load_table_symbol)?;
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
        images.len() == palettes.len()
            && images
                .get(consumer.image_load_index)
                .is_some_and(|record| record.file_record == consumer.image_file_record)
            && palettes
                .get(consumer.palette_slot)
                .is_some_and(|record| record.file_record == consumer.palette_file_record),
        "unreferenced source load-table image or palette route changed"
    );

    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let overlapping_pattern_indices = patterns
        .iter()
        .enumerate()
        .filter_map(|(index, pattern)| {
            let bounds = PixelBounds {
                x: pattern.source_x,
                y: pattern.source_y,
                width: pattern.width,
                height: pattern.height,
            };
            (pattern.image_load_index == consumer.pattern_texture_slot
                && pixel_bounds_overlap(bounds, consumer.source_bounds))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    ensure!(
        overlapping_pattern_indices.is_empty(),
        "unreferenced source region overlaps declared pattern records: {overlapping_pattern_indices:?}"
    );

    let direct_sequence_pattern_references = collect_sequence_pattern_references(
        file,
        &consumer.sequence_symbol,
        consumer.create_pattern_opcode,
        consumer.update_pattern_opcode,
    )?;
    let animation_pattern_references = collect_animation_pattern_references(
        file,
        &consumer.animation_pointer_table_symbol,
        &consumer.animation_table_symbols,
    )?;
    ensure!(
        direct_sequence_pattern_references
            .iter()
            .chain(&animation_pattern_references)
            .all(|index| *index < patterns.len()),
        "unreferenced source route selects a pattern outside its table"
    );

    Ok(StylizedArtworkUnreferencedSourceConsumerValidation {
        load_table_symbol: consumer.load_table_symbol.clone(),
        image_load_index: consumer.image_load_index,
        image_file_record: consumer.image_file_record,
        palette_slot: consumer.palette_slot,
        palette_file_record: consumer.palette_file_record,
        pattern_table_symbol: consumer.pattern_table_symbol.clone(),
        pattern_texture_slot: consumer.pattern_texture_slot,
        source_bounds: consumer.source_bounds,
        sequence_symbol: consumer.sequence_symbol.clone(),
        animation_pointer_table_symbol: consumer.animation_pointer_table_symbol.clone(),
        pattern_record_count: patterns.len(),
        direct_sequence_pattern_references,
        animation_pattern_references,
        runtime_status: "no pattern rectangle selects this source region in the declared demo route",
    })
}

pub(in crate::graphics_translation::stylized_artwork) fn collect_sequence_pattern_references(
    file: &object::File<'_>,
    sequence_symbol: &str,
    create_pattern_opcode: usize,
    update_pattern_opcode: usize,
) -> Result<Vec<usize>> {
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
            .get(create_pattern_opcode)
            .is_some_and(|words| *words >= 7)
            && command_sizes
                .get(update_pattern_opcode)
                .is_some_and(|words| *words >= 3),
        "source pattern sequence command sizes changed"
    );

    let (_, sequence_bytes) = read_symbol_bytes(file, sequence_symbol)?;
    let mut references = BTreeSet::new();
    let mut offset = 0;
    while offset < sequence_bytes.len() {
        let opcode = usize::from(u16::from_le_bytes(
            sequence_bytes
                .get(offset..offset + 2)
                .with_context(|| format!("{sequence_symbol} ends inside a sequence opcode"))?
                .try_into()?,
        ));
        let words = *command_sizes.get(opcode).with_context(|| {
            format!("{sequence_symbol} uses opcode {opcode:#x} outside SeqComSize")
        })?;
        ensure!(
            words > 0,
            "{sequence_symbol} uses zero-sized opcode {opcode:#x}"
        );
        let byte_length = words
            .checked_mul(2)
            .context("source pattern sequence command length overflow")?;
        let command = sequence_bytes
            .get(offset..offset + byte_length)
            .with_context(|| format!("{sequence_symbol} ends inside opcode {opcode:#x}"))?;
        if opcode == create_pattern_opcode {
            references.insert(usize::from(u16::from_le_bytes(command[12..14].try_into()?)));
        } else if opcode == update_pattern_opcode {
            references.insert(usize::from(u16::from_le_bytes(command[4..6].try_into()?)));
        }
        offset += byte_length;
    }
    ensure!(
        offset == sequence_bytes.len(),
        "{sequence_symbol} command parsing did not end at the symbol boundary"
    );
    Ok(references.into_iter().collect())
}

pub(in crate::graphics_translation::stylized_artwork) fn collect_animation_pattern_references(
    file: &object::File<'_>,
    animation_pointer_table_symbol: &str,
    animation_table_symbols: &[String],
) -> Result<Vec<usize>> {
    validate_pointer_table_symbols(
        file,
        animation_pointer_table_symbol,
        animation_table_symbols,
    )?;
    let mut references = BTreeSet::new();
    for symbol in animation_table_symbols {
        let (_, animation_bytes) = read_symbol_bytes(file, symbol)?;
        ensure!(
            !animation_bytes.is_empty() && animation_bytes.len().is_multiple_of(4),
            "source pattern animation table is not a nonempty sequence of four-byte records: {symbol}"
        );
        for record in animation_bytes.chunks_exact(4) {
            let pattern_index = u16::from_le_bytes(record[0..2].try_into()?);
            if pattern_index < 0xfffa {
                references.insert(usize::from(pattern_index));
            }
        }
    }
    Ok(references.into_iter().collect())
}
