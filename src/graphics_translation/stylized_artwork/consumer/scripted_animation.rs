use super::*;

const ANIMATION_RECORD_BYTES: usize = 4;
const ANIMATION_FIRST_CONTROL: u16 = 0xfff9;
const ANIMATION_TERMINATOR: u16 = u16::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AnimationScriptRecord {
    operation: u16,
    operand: u16,
}

pub(in crate::graphics_translation::stylized_artwork) fn validate_scripted_animation_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    native_placement: PixelBounds,
    consumer: &StylizedArtworkScriptedAnimationConsumer,
    region: &super::GraphicsTextRegion,
    legacy_demo_registration_route: &StylizedArtworkLegacyDemoRegistrationRoute,
) -> Result<StylizedArtworkScriptedAnimationConsumerValidation> {
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
        "scripted-animation consumer disagrees with its legacy demo registration route"
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
        "scripted-animation load-table image or palette route changed"
    );

    let (_, pattern_bytes) = read_symbol_bytes(file, &consumer.pattern_table_symbol)?;
    let patterns = parse_pattern_records(&consumer.pattern_table_symbol, &pattern_bytes)?;
    let pattern = patterns
        .get(consumer.pattern_record_index)
        .context("scripted-animation pattern record is out of range")?;
    ensure!(
        pattern.image_load_index == consumer.pattern_texture_slot
            && pattern.source_x == consumer.pattern_source_bounds.x
            && pattern.source_y == consumer.pattern_source_bounds.y
            && pattern.width == consumer.pattern_source_bounds.width
            && pattern.height == consumer.pattern_source_bounds.height,
        "scripted-animation pattern record changed"
    );
    ensure_bounds_contains(
        consumer.pattern_source_bounds,
        region.bounds,
        "scripted-animation Japanese region exceeds its pattern source rectangle",
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
            .get(consumer.create_opcode)
            .is_some_and(|words| *words == consumer.create_command_words)
            && command_sizes
                .get(consumer.animation_opcode)
                .is_some_and(|words| *words == consumer.animation_command_words),
        "scripted-animation sequence command size changed"
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
        "scripted-animation actor-creation command changed"
    );
    ensure!(
        patterns
            .get(consumer.initial_pattern_record_index)
            .is_some_and(|initial| initial.image_load_index == consumer.pattern_texture_slot),
        "scripted-animation initial actor pattern changed"
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
        "scripted-animation selection command changed"
    );

    let animation_pointers =
        read_pointer_table_addresses(file, &consumer.animation_pointer_table_symbol)?;
    ensure!(
        animation_pointers.get(consumer.animation_index).copied()
            == Some(symbol_address(file, &consumer.animation_table_symbol)?),
        "scripted-animation pointer changed"
    );
    let (_, animation_bytes) = read_symbol_bytes(file, &consumer.animation_table_symbol)?;
    let animation_records = parse_animation_script(
        &animation_bytes,
        patterns.len(),
        consumer.animation_terminator,
        consumer.animation_terminator_operand,
    )?;
    let selected_record = animation_records
        .get(consumer.animation_script_record_index)
        .context("scripted-animation selected record is out of range")?;
    ensure!(
        selected_record.operation == u16::try_from(consumer.pattern_record_index)?
            && selected_record.operand == consumer.animation_frame_duration,
        "scripted-animation selected frame changed"
    );

    let logical_bounds = derive_screen_bounds(
        consumer.pattern_screen_origin,
        consumer.pattern_source_bounds,
        region.bounds,
    )?;
    ensure!(
        logical_bounds == consumer.derived_logical_screen_bounds,
        "scripted-animation logical screen bounds changed"
    );
    let derived_native_screen_bounds = derive_screen_bounds(
        consumer.pattern_screen_origin,
        consumer.pattern_source_bounds,
        native_placement,
    )?;
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
    let animation_frame_records = animation_records
        .iter()
        .filter(|record| usize::from(record.operation) < patterns.len())
        .count();
    let animation_control_records = animation_records.len() - animation_frame_records - 1;

    Ok(StylizedArtworkScriptedAnimationConsumerValidation {
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
        animation_script_record_index: consumer.animation_script_record_index,
        animation_frame_duration: consumer.animation_frame_duration,
        animation_script_records: animation_records.len(),
        animation_frame_records,
        animation_control_records,
        animation_terminator: consumer.animation_terminator,
        animation_terminator_operand: consumer.animation_terminator_operand,
        pattern_screen_origin: consumer.pattern_screen_origin,
        pattern_depth: consumer.pattern_depth,
        derived_logical_screen_bounds: logical_bounds,
        derived_native_screen_bounds,
        typed_create_decoder_instructions,
        typed_animation_decoder_instructions,
        runtime_status: "not observed for this exact artwork overlay",
    })
}

fn parse_animation_script(
    source: &[u8],
    pattern_record_count: usize,
    expected_terminator: u16,
    expected_terminator_operand: u16,
) -> Result<Vec<AnimationScriptRecord>> {
    ensure!(
        source.len().is_multiple_of(ANIMATION_RECORD_BYTES) && !source.is_empty(),
        "animation script is not a nonempty sequence of four-byte records"
    );
    let records = source
        .chunks_exact(ANIMATION_RECORD_BYTES)
        .map(|record| AnimationScriptRecord {
            operation: u16::from_le_bytes(record[0..2].try_into().expect("two-byte operation")),
            operand: u16::from_le_bytes(record[2..4].try_into().expect("two-byte operand")),
        })
        .collect::<Vec<_>>();
    ensure!(
        (ANIMATION_FIRST_CONTROL..=ANIMATION_TERMINATOR).contains(&expected_terminator),
        "animation script terminator is outside the control range"
    );
    ensure!(
        records.last().copied()
            == Some(AnimationScriptRecord {
                operation: expected_terminator,
                operand: expected_terminator_operand,
            }),
        "animation script has no exact terminator"
    );
    for record in &records[..records.len() - 1] {
        ensure!(
            (usize::from(record.operation) < pattern_record_count
                || (ANIMATION_FIRST_CONTROL..ANIMATION_TERMINATOR).contains(&record.operation))
                && record.operation != expected_terminator,
            "animation script contains an invalid pattern or control operation"
        );
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frames_controls_and_one_terminal_record() {
        let source = [
            0xfb, 0xff, 0x00, 0x00, 0x11, 0x00, 0x1e, 0x00, 0xfa, 0xff, 0x03, 0x00, 0xff, 0xff,
            0x00, 0x00,
        ];
        let records = parse_animation_script(&source, 30, ANIMATION_TERMINATOR, 0).unwrap();
        assert_eq!(records.len(), 4);
        assert_eq!(records[1].operation, 17);
        assert_eq!(records[1].operand, 30);
    }

    #[test]
    fn rejects_an_early_terminal_record() {
        let source = [
            0xff, 0xff, 0x00, 0x00, 0x11, 0x00, 0x1e, 0x00, 0xff, 0xff, 0x00, 0x00,
        ];
        assert!(parse_animation_script(&source, 30, ANIMATION_TERMINATOR, 0).is_err());
    }

    #[test]
    fn accepts_a_loop_control_as_the_declared_terminal_record() {
        let source = [
            0x02, 0x00, 0x1e, 0x00, 0x03, 0x00, 0x0c, 0x00, 0xfe, 0xff, 0x00, 0x00,
        ];
        let records = parse_animation_script(&source, 30, 0xfffe, 0).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[1].operation, 3);
    }

    #[test]
    fn rejects_the_declared_loop_terminator_before_the_last_record() {
        let source = [
            0xfe, 0xff, 0x00, 0x00, 0x03, 0x00, 0x0c, 0x00, 0xfe, 0xff, 0x00, 0x00,
        ];
        assert!(parse_animation_script(&source, 30, 0xfffe, 0).is_err());
    }

    #[test]
    fn preserves_a_loop_target_operand_on_the_terminal_record() {
        let source = [
            0x07, 0x00, 0x42, 0x00, 0x08, 0x00, 0x09, 0x00, 0xfe, 0xff, 0x01, 0x00,
        ];
        let records = parse_animation_script(&source, 30, 0xfffe, 1).unwrap();
        assert_eq!(records.last().unwrap().operand, 1);
    }
}
