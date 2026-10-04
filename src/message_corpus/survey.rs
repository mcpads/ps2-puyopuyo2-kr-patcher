use super::catalog::*;
use super::instruction::*;
use super::parsing::*;
use super::*;

pub fn survey_memory_card_messages(inputs: MessageCorpusInputs<'_>) -> Result<MessageCorpusReport> {
    survey_message_consumer(inputs, LOAD_CONSUMER)
}

pub fn survey_memory_card_save_messages(
    inputs: MessageCorpusInputs<'_>,
) -> Result<MessageCorpusReport> {
    survey_message_consumer(inputs, SAVE_CONSUMER)
}

pub fn survey_my_nazo_error_messages(
    inputs: MessageCorpusInputs<'_>,
) -> Result<MessageCorpusReport> {
    survey_message_consumer(inputs, MY_NAZO_ERROR_CONSUMER)
}

fn survey_tutorial_message_bank(executable: &[u8]) -> Result<TutorialMessageBankEvidence> {
    let start = read_executable_symbol(executable, TUTORIAL_MESSAGES[0].symbol)?;
    let table = read_executable_symbol(executable, TUTORIAL_MESSAGE_TABLE_SYMBOL)?;
    let byte_length = usize::try_from(
        table
            .address
            .checked_sub(start.address)
            .context("tutorial message table precedes the message bank")?,
    )?;
    ensure!(
        table.file_offset == start.file_offset + byte_length,
        "tutorial message bank is not contiguous with ExpMsg_Data"
    );
    let bank = executable
        .get(start.file_offset..table.file_offset)
        .context("tutorial message bank exceeds the executable")?;
    let mut declared = vec![false; byte_length];
    let mut declared_message_bytes = 0_usize;
    for message in TUTORIAL_MESSAGES {
        let symbol = read_executable_symbol(executable, message.symbol)?;
        let relative_address =
            usize::try_from(symbol.address.checked_sub(start.address).with_context(|| {
                format!("{} precedes the tutorial message bank", message.symbol)
            })?)?;
        let relative_file_offset = symbol
            .file_offset
            .checked_sub(start.file_offset)
            .with_context(|| format!("{} precedes the tutorial message bank", message.symbol))?;
        ensure!(
            relative_address == relative_file_offset
                && relative_address + symbol.bytes.len() <= byte_length,
            "{} is not wholly contained in the tutorial message bank",
            message.symbol
        );
        for covered in &mut declared[relative_address..relative_address + symbol.bytes.len()] {
            ensure!(!*covered, "tutorial message symbols overlap");
            *covered = true;
        }
        declared_message_bytes += symbol.bytes.len();
    }
    let padding_offsets = declared
        .iter()
        .enumerate()
        .filter_map(|(offset, covered)| (!covered).then_some(offset))
        .collect::<Vec<_>>();
    ensure!(
        padding_offsets.iter().all(|offset| bank[*offset] == 0),
        "tutorial message bank contains nonzero bytes outside declared message symbols"
    );
    Ok(TutorialMessageBankEvidence {
        start_symbol: TUTORIAL_MESSAGES[0].symbol,
        address: format!("{:#010x}", start.address),
        file_offset: format!("{:#010x}", start.file_offset),
        byte_length,
        source_sha256: sha256(bank),
        declared_message_bytes,
        zero_padding_bytes: padding_offsets.len(),
        message_count: TUTORIAL_MESSAGES.len(),
    })
}

fn survey_tutorial_message_layout(
    file: &object::File<'_>,
) -> Result<TutorialMessageLayoutEvidence> {
    let (_, command_size_bytes) = read_symbol_bytes(file, TUTORIAL_COMMAND_SIZE_SYMBOL)?;
    ensure!(
        command_size_bytes.len().is_multiple_of(2),
        "SeqComSize is not a sequence of 16-bit sizes"
    );
    let command_sizes = command_size_bytes
        .chunks_exact(2)
        .map(|bytes| usize::from(u16::from_le_bytes([bytes[0], bytes[1]])))
        .collect::<Vec<_>>();
    let mut commands = Vec::new();
    for sequence_symbol in TUTORIAL_SEQUENCE_SYMBOLS {
        let (_, source) = read_symbol_bytes(file, sequence_symbol)?;
        let mut offset = 0_usize;
        while offset < source.len() {
            let opcode = read_sequence_word(sequence_symbol, &source, offset)?;
            let command_words = *command_sizes.get(opcode).with_context(|| {
                format!("{sequence_symbol} uses opcode {opcode:#x} outside SeqComSize")
            })?;
            ensure!(
                command_words > 0,
                "{sequence_symbol} uses zero-sized opcode {opcode:#x}"
            );
            let command_bytes = command_words
                .checked_mul(2)
                .context("tutorial sequence command length overflow")?;
            let command = source
                .get(offset..offset + command_bytes)
                .with_context(|| format!("{sequence_symbol} ends inside opcode {opcode:#x}"))?;
            if opcode == TUTORIAL_MESSAGE_COMMAND_OPCODE {
                ensure!(
                    command_words == TUTORIAL_MESSAGE_COMMAND_WORDS,
                    "tutorial message command size changed"
                );
                commands.push(TutorialMessageCommandEvidence {
                    sequence_symbol,
                    byte_offset: format!("{offset:#x}"),
                    actor: read_sequence_word(sequence_symbol, command, 2)?,
                    message_index: read_sequence_word(sequence_symbol, command, 4)?,
                    x_pixels: read_sequence_word(sequence_symbol, command, 6)?,
                    y_pixels: read_sequence_word(sequence_symbol, command, 8)?,
                    depth: read_sequence_word(sequence_symbol, command, 10)?,
                    reveal_rate: read_sequence_word(sequence_symbol, command, 12)?,
                });
            }
            offset += command_bytes;
        }
    }
    const EXPECTED_COMMANDS: [(&str, usize, usize); 12] = [
        ("ExpDemo_0", 0x5c, 0),
        ("ExpDemo_0", 0x94, 1),
        ("ExpDemo_0", 0xde, 2),
        ("ExpDemo_0", 0x11e, 3),
        ("ExpDemo_0", 0x158, 4),
        ("ExpDemo_0", 0x1a4, 5),
        ("ExpDemo_0", 0x1e0, 6),
        ("ExpDemo_0", 0x22c, 7),
        ("ExpDemo_0", 0x27c, 8),
        ("ExpDemo_1", 0x32, 9),
        ("ExpDemo_1", 0x70, 10),
        ("ExpDemo_1", 0xb8, 11),
    ];
    ensure!(
        commands.len() == EXPECTED_COMMANDS.len(),
        "tutorial message command population changed: expected {}, got {} at {:?}",
        EXPECTED_COMMANDS.len(),
        commands.len(),
        commands
            .iter()
            .map(|command| (command.sequence_symbol, command.byte_offset.as_str()))
            .collect::<Vec<_>>()
    );
    for (command, (sequence_symbol, offset, message_index)) in
        commands.iter().zip(EXPECTED_COMMANDS)
    {
        ensure!(
            command.sequence_symbol == sequence_symbol
                && command.byte_offset == format!("{offset:#x}")
                && command.message_index == message_index
                && command.actor == 1
                && command.x_pixels == 336
                && command.y_pixels == 96
                && command.depth == 100
                && command.reveal_rate == 4,
            "tutorial message command layout changed at {sequence_symbol}+{offset:#x}"
        );
    }
    let unique_message_indices = commands
        .iter()
        .map(|command| command.message_index)
        .collect::<BTreeSet<_>>();
    ensure!(
        unique_message_indices == (0..TUTORIAL_MESSAGES.len()).collect(),
        "tutorial sequence message-index coverage changed"
    );
    let origin_x_pixels = commands[0].x_pixels;
    let origin_y_pixels = commands[0].y_pixels;
    let line_cell_capacity =
        (TUTORIAL_VIEWPORT_WIDTH_PIXELS - origin_x_pixels) / TUTORIAL_GLYPH_WIDTH_PIXELS;
    let line_capacity =
        (TUTORIAL_VIEWPORT_HEIGHT_PIXELS - origin_y_pixels - TUTORIAL_GLYPH_HEIGHT_PIXELS)
            / TUTORIAL_LINE_ADVANCE_PIXELS
            + 1;
    ensure!(
        line_cell_capacity == 9
            && origin_x_pixels + line_cell_capacity * TUTORIAL_GLYPH_WIDTH_PIXELS
                <= TUTORIAL_VIEWPORT_WIDTH_PIXELS
            && origin_x_pixels + (line_cell_capacity + 1) * TUTORIAL_GLYPH_WIDTH_PIXELS
                > TUTORIAL_VIEWPORT_WIDTH_PIXELS,
        "tutorial horizontal capacity is no longer nine fullwidth cells"
    );
    ensure!(
        line_capacity == 8
            && origin_y_pixels
                + (line_capacity - 1) * TUTORIAL_LINE_ADVANCE_PIXELS
                + TUTORIAL_GLYPH_HEIGHT_PIXELS
                <= TUTORIAL_VIEWPORT_HEIGHT_PIXELS
            && origin_y_pixels
                + line_capacity * TUTORIAL_LINE_ADVANCE_PIXELS
                + TUTORIAL_GLYPH_HEIGHT_PIXELS
                > TUTORIAL_VIEWPORT_HEIGHT_PIXELS,
        "tutorial vertical capacity is no longer eight lines"
    );
    Ok(TutorialMessageLayoutEvidence {
        command_opcode: format!("{TUTORIAL_MESSAGE_COMMAND_OPCODE:#x}"),
        sequence_symbols: TUTORIAL_SEQUENCE_SYMBOLS.to_vec(),
        physical_message_commands: commands.len(),
        unique_message_indices: unique_message_indices.len(),
        repeated_message_commands: commands.len() - unique_message_indices.len(),
        actor: commands[0].actor,
        origin_x_pixels,
        origin_y_pixels,
        depth: commands[0].depth,
        reveal_rate: commands[0].reveal_rate,
        viewport_width_pixels: TUTORIAL_VIEWPORT_WIDTH_PIXELS,
        viewport_height_pixels: TUTORIAL_VIEWPORT_HEIGHT_PIXELS,
        glyph_width_pixels: TUTORIAL_GLYPH_WIDTH_PIXELS,
        glyph_height_pixels: TUTORIAL_GLYPH_HEIGHT_PIXELS,
        line_advance_pixels: TUTORIAL_LINE_ADVANCE_PIXELS,
        line_cell_capacity,
        line_capacity,
        commands,
    })
}

fn read_sequence_word(symbol: &str, source: &[u8], offset: usize) -> Result<usize> {
    let bytes = source
        .get(offset..offset + 2)
        .with_context(|| format!("{symbol} ends inside a 16-bit sequence word"))?;
    Ok(usize::from(u16::from_le_bytes([bytes[0], bytes[1]])))
}

pub fn survey_tutorial_messages(
    inputs: MessageCorpusInputs<'_>,
) -> Result<TutorialMessageCorpusReport> {
    ensure!(
        sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    let file = object::File::parse(inputs.executable).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips,
        "ELF architecture is not 32-bit MIPS"
    );
    ensure!(
        file.endianness() == Endianness::Little,
        "ELF is not little-endian"
    );

    let producer_disassembly = disassemble_symbol(inputs.executable, TUTORIAL_PRODUCER_SYMBOL)?;
    let renderer_disassembly = disassemble_symbol(inputs.executable, TUTORIAL_RENDERER_SYMBOL)?;
    let lookup_disassembly = disassemble_symbol(inputs.executable, TUTORIAL_LOOKUP_SYMBOL)?;
    let (producer_address, producer_bytes) = read_symbol_bytes(&file, TUTORIAL_PRODUCER_SYMBOL)?;
    let (renderer_address, renderer_bytes) = read_symbol_bytes(&file, TUTORIAL_RENDERER_SYMBOL)?;
    let (lookup_address, lookup_bytes) = read_symbol_bytes(&file, TUTORIAL_LOOKUP_SYMBOL)?;
    let (setter_address, _) = read_symbol_bytes(&file, ACTOR_SETTER_SYMBOL)?;
    let (message_table_address, message_table_bytes) =
        read_symbol_bytes(&file, TUTORIAL_MESSAGE_TABLE_SYMBOL)?;
    let message_table_symbol =
        read_executable_symbol(inputs.executable, TUTORIAL_MESSAGE_TABLE_SYMBOL)?;
    let (font_table_address, font_table_bytes) =
        read_symbol_bytes(&file, TUTORIAL_FONT_TABLE_SYMBOL)?;
    let message_bank = survey_tutorial_message_bank(inputs.executable)?;
    let layout = survey_tutorial_message_layout(&file)?;

    let actor_creations =
        find_renderer_actor_creation_sites(&file, renderer_address, setter_address)?;
    ensure!(
        actor_creations
            == [RendererActorCreationSite {
                producer_symbol: TUTORIAL_PRODUCER_SYMBOL.to_string(),
                producer_address,
                actor: TUTORIAL_TABLE_REFERENCE.actor,
            }],
        "ExpMsgPrint_Act producer population differs from the fixed SeqCommand branch"
    );

    let reference = TUTORIAL_TABLE_REFERENCE;
    let mut reference_instructions = validate_actor_creation(
        &producer_bytes,
        producer_address,
        reference.actor,
        renderer_address,
        TUTORIAL_RENDERER_SYMBOL,
        setter_address,
    )?;
    reference_instructions.extend(validate_loaded_address(
        &producer_bytes,
        producer_address,
        reference.upper_offset,
        reference.lower_offset,
        reference.table_register,
        message_table_address,
        reference.table_symbol,
    )?);
    reference_instructions.push(validate_tutorial_table_index_load(
        &producer_bytes,
        producer_address,
        0x1684,
    )?);
    reference_instructions.extend(validate_table_lookup(
        &producer_bytes,
        producer_address,
        reference,
    )?);
    reference_instructions.push(validate_message_store(
        &producer_bytes,
        producer_address,
        reference.store_offset,
        reference.message_register,
        reference.store_base,
    )?);
    reference_instructions.sort_by(|left, right| left.address.cmp(&right.address));
    validate_pointer_table(&file, reference, &message_table_bytes)?;

    let font_table = parse_tutorial_font_table(&font_table_bytes)?;
    let mut messages = Vec::with_capacity(TUTORIAL_MESSAGES.len());
    for message in TUTORIAL_MESSAGES {
        let entry = read_message_entry(&file, message, vec![TUTORIAL_REFERENCE_ID.to_string()])?;
        validate_tutorial_message_tokens(&entry, &font_table.unique_code_units)?;
        messages.push(entry);
    }
    messages.sort_by(|left, right| left.source_address.cmp(&right.source_address));

    let raw_payload_bytes = messages
        .iter()
        .map(|message| message.payload_byte_length)
        .sum();
    let japanese_characters = messages
        .iter()
        .flat_map(|message| &message.tokens)
        .filter(|token| token.kind == "shift-jis-text")
        .map(|token| token.value.chars().count())
        .sum();
    let line_breaks = messages
        .iter()
        .flat_map(|message| &message.tokens)
        .filter(|token| token.kind == "line-break")
        .count();
    let protected_english_runs = messages
        .iter()
        .map(|message| message.protected_english_runs.len())
        .sum();
    ensure!(
        protected_english_runs == 0,
        "tutorial corpus unexpectedly contains protected English text"
    );

    Ok(TutorialMessageCorpusReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        scope: MessageCorpusScope {
            consumer_symbol: TUTORIAL_PRODUCER_SYMBOL,
            renderer_symbol: TUTORIAL_RENDERER_SYMBOL,
            included: "the complete repackable message bank, ExpMsg_Data pointer table, and ExpDemo_0/1 message commands selected by the sole ExpMsgPrint_Act producer branch",
            excluded: "NazoMsgPrint_Act, MsgPrint_Act, and every other text renderer",
            runtime_status: "static producer and renderer proof only; no cold-boot screen proof",
        },
        population: MessagePopulation {
            status: "complete for the declared ExpMsgPrint_Act producer and ExpMsg_Data table population",
            consumer_message_sites: 1,
            direct_sites: 0,
            indexed_table_sites: 1,
            physical_reference_edges: TUTORIAL_MESSAGES.len(),
            unique_messages: messages.len(),
            duplicate_reference_edges: 0,
            unresolved_sites: 0,
            raw_payload_bytes,
            japanese_characters,
            protected_english_runs,
        },
        producer: TutorialMessageProducerEvidence {
            isa_profile: r5900::PROFILE_ID,
            symbol: TUTORIAL_PRODUCER_SYMBOL,
            address: format!("{producer_address:#010x}"),
            byte_length: producer_bytes.len(),
            sha256: sha256(&producer_bytes),
            instruction_count: producer_disassembly.instructions.len(),
            renderer_actor_creations: actor_creations.len(),
            message_reference: MessageReference {
                reference_id: TUTORIAL_REFERENCE_ID.to_string(),
                kind: "indexed-pointer-table",
                table_symbol: Some(TUTORIAL_MESSAGE_TABLE_SYMBOL.to_string()),
                message_ids: TUTORIAL_MESSAGES
                    .iter()
                    .map(|message| message.id.to_string())
                    .collect(),
                message_symbols: TUTORIAL_MESSAGES
                    .iter()
                    .map(|message| message.symbol.to_string())
                    .collect(),
                instructions: reference_instructions,
            },
        },
        renderer: TutorialMessageRendererEvidence {
            symbol: TUTORIAL_RENDERER_SYMBOL,
            address: format!("{renderer_address:#010x}"),
            byte_length: renderer_bytes.len(),
            sha256: sha256(&renderer_bytes),
            instruction_count: renderer_disassembly.instructions.len(),
            invariants: validate_tutorial_renderer(
                &renderer_bytes,
                renderer_address,
                lookup_address,
            )?,
        },
        message_bank,
        message_table: TutorialMessageTableEvidence {
            symbol: TUTORIAL_MESSAGE_TABLE_SYMBOL,
            address: format!("{message_table_address:#010x}"),
            file_offset: format!("{:#010x}", message_table_symbol.file_offset),
            byte_length: message_table_bytes.len(),
            sha256: sha256(&message_table_bytes),
            pointer_count: TUTORIAL_MESSAGES.len(),
        },
        layout,
        font_lookup: TutorialFontLookupEvidence {
            symbol: TUTORIAL_LOOKUP_SYMBOL,
            address: format!("{lookup_address:#010x}"),
            byte_length: lookup_bytes.len(),
            sha256: sha256(&lookup_bytes),
            instruction_count: lookup_disassembly.instructions.len(),
            font_table_symbol: TUTORIAL_FONT_TABLE_SYMBOL,
            font_table_address: format!("{font_table_address:#010x}"),
            font_table_byte_length: font_table_bytes.len(),
            font_table_sha256: sha256(&font_table_bytes),
            stored_code_slots: font_table.code_slots.len(),
            unique_code_units: font_table.unique_code_units.len(),
            duplicate_code_slots: font_table.code_slots.len() - font_table.unique_code_units.len(),
            repeated_code_units: font_table.repeated_code_units,
            invariants: validate_tutorial_font_lookup(
                &lookup_bytes,
                lookup_address,
                font_table_address,
            )?,
        },
        control_codes: vec![ControlCodeUsage {
            kind: "line-break".to_string(),
            raw_values: vec!["0a".to_string()],
            occurrences: line_breaks,
            byte_width: 1,
            policy: "movable-layout".to_string(),
            confirmed_effect: "advance the input by one byte, reset x, and advance y by 48 pixels"
                .to_string(),
        }],
        messages,
        translation_scope: MessageTranslationScope {
            target: "non-ASCII Shift-JIS runs in Japanese tutorial messages",
            preserve: "every value outside the declared tutorial message bank and pointer table; Japanese line breaks and per-symbol storage boundaries carry no Korean layout authority",
            report_role: "read-only corpus evidence; not an approved translation asset",
        },
    })
}

fn survey_message_consumer(
    inputs: MessageCorpusInputs<'_>,
    spec: MessageConsumerSpec,
) -> Result<MessageCorpusReport> {
    ensure!(
        sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    let file = object::File::parse(inputs.executable).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips,
        "ELF architecture is not 32-bit MIPS"
    );
    ensure!(
        file.endianness() == Endianness::Little,
        "ELF is not little-endian"
    );

    let consumer_disassembly = disassemble_symbol(inputs.executable, spec.consumer_symbol)?;
    let renderer_disassembly = disassemble_symbol(inputs.executable, RENDERER_SYMBOL)?;
    let (consumer_address, consumer_bytes) = read_symbol_bytes(&file, spec.consumer_symbol)?;
    let (renderer_address, renderer_bytes) = read_symbol_bytes(&file, RENDERER_SYMBOL)?;
    let (setter_address, _) = read_symbol_bytes(&file, ACTOR_SETTER_SYMBOL)?;

    let expected_store_offsets: BTreeSet<usize> = spec
        .direct_references
        .iter()
        .map(|reference| reference.store_offset)
        .chain(
            spec.table_references
                .iter()
                .map(|reference| reference.store_offset),
        )
        .collect();
    ensure!(
        expected_store_offsets.len() == spec.direct_references.len() + spec.table_references.len(),
        "{} message reference catalog contains duplicate consumer stores",
        spec.consumer_symbol
    );
    let (nonnull_stores, null_stores) =
        collect_message_store_offsets(&consumer_bytes, consumer_address)?;
    ensure!(
        nonnull_stores == expected_store_offsets,
        "{} message-store population differs from the fixed catalog",
        spec.consumer_symbol
    );
    ensure!(
        null_stores == spec.expected_null_stores,
        "{} null message-store count changed",
        spec.consumer_symbol
    );

    let mut references =
        Vec::with_capacity(spec.direct_references.len() + spec.table_references.len());
    let mut messages_by_symbol = BTreeMap::<&str, (MessageSpec, Vec<String>)>::new();
    for reference in spec.direct_references.iter().copied() {
        let mut instructions = validate_actor_creation(
            &consumer_bytes,
            consumer_address,
            reference.actor,
            renderer_address,
            RENDERER_SYMBOL,
            setter_address,
        )?;
        let (message_address, _) = read_symbol_bytes(&file, reference.message.symbol)?;
        instructions.extend(validate_loaded_address(
            &consumer_bytes,
            consumer_address,
            reference.upper_offset,
            reference.lower_offset,
            reference.value_register,
            message_address,
            reference.message.symbol,
        )?);
        instructions.push(validate_message_store(
            &consumer_bytes,
            consumer_address,
            reference.store_offset,
            reference.value_register,
            reference.store_base,
        )?);
        instructions.sort_by(|left, right| left.address.cmp(&right.address));
        register_message_reference(
            &mut messages_by_symbol,
            reference.message,
            reference.reference_id,
        )?;
        references.push(MessageReference {
            reference_id: reference.reference_id.to_string(),
            kind: "direct",
            table_symbol: None,
            message_ids: vec![reference.message.id.to_string()],
            message_symbols: vec![reference.message.symbol.to_string()],
            instructions,
        });
    }
    for reference in spec.table_references.iter().copied() {
        let mut instructions = validate_actor_creation(
            &consumer_bytes,
            consumer_address,
            reference.actor,
            renderer_address,
            RENDERER_SYMBOL,
            setter_address,
        )?;
        let (table_address, table_bytes) = read_symbol_bytes(&file, reference.table_symbol)?;
        instructions.extend(validate_loaded_address(
            &consumer_bytes,
            consumer_address,
            reference.upper_offset,
            reference.lower_offset,
            reference.table_register,
            table_address,
            reference.table_symbol,
        )?);
        instructions.extend(validate_table_lookup(
            &consumer_bytes,
            consumer_address,
            reference,
        )?);
        instructions.push(validate_message_store(
            &consumer_bytes,
            consumer_address,
            reference.store_offset,
            reference.message_register,
            reference.store_base,
        )?);
        validate_pointer_table(&file, reference, &table_bytes)?;
        instructions.sort_by(|left, right| left.address.cmp(&right.address));
        for message in reference.messages {
            register_message_reference(&mut messages_by_symbol, *message, reference.reference_id)?;
        }
        references.push(MessageReference {
            reference_id: reference.reference_id.to_string(),
            kind: "indexed-pointer-table",
            table_symbol: Some(reference.table_symbol.to_string()),
            message_ids: reference
                .messages
                .iter()
                .map(|message| message.id.to_string())
                .collect(),
            message_symbols: reference
                .messages
                .iter()
                .map(|message| message.symbol.to_string())
                .collect(),
            instructions,
        });
    }
    references.sort_by_key(|reference| {
        reference
            .instructions
            .iter()
            .map(|instruction| instruction.address.as_str())
            .min()
            .unwrap_or("")
            .to_string()
    });

    let mut messages = Vec::with_capacity(messages_by_symbol.len());
    for (_, (spec, mut reference_ids)) in messages_by_symbol {
        reference_ids.sort();
        messages.push(read_message_entry(&file, spec, reference_ids)?);
    }
    messages.sort_by(|left, right| left.source_address.cmp(&right.source_address));

    let mut control_usage = BTreeMap::<String, ControlAccumulator>::new();
    let mut raw_payload_bytes = 0_usize;
    let mut japanese_characters = 0_usize;
    let mut protected_english_runs = 0_usize;
    for message in &messages {
        raw_payload_bytes += message.payload_byte_length;
        protected_english_runs += message.protected_english_runs.len();
        for token in &message.tokens {
            if token.kind == "shift-jis-text" {
                japanese_characters += token.value.chars().count();
            } else if token.kind != "ascii-text" {
                let accumulator = control_usage
                    .entry(token.kind.to_string())
                    .or_insert_with(|| ControlAccumulator::from_token(token));
                accumulator.record(token);
            }
        }
    }
    let controls = control_usage
        .into_values()
        .map(ControlAccumulator::finish)
        .collect();
    let physical_reference_edges = spec.direct_references.len()
        + spec
            .table_references
            .iter()
            .map(|reference| reference.messages.len())
            .sum::<usize>();

    Ok(MessageCorpusReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        scope: MessageCorpusScope {
            consumer_symbol: spec.consumer_symbol,
            renderer_symbol: RENDERER_SYMBOL,
            included: spec.included,
            excluded: spec.excluded,
            runtime_status: "static consumer proof only; no cold-boot screen proof",
        },
        population: MessagePopulation {
            status: spec.population_status,
            consumer_message_sites: expected_store_offsets.len(),
            direct_sites: spec.direct_references.len(),
            indexed_table_sites: spec.table_references.len(),
            physical_reference_edges,
            unique_messages: messages.len(),
            duplicate_reference_edges: physical_reference_edges - messages.len(),
            unresolved_sites: 0,
            raw_payload_bytes,
            japanese_characters,
            protected_english_runs,
        },
        consumer: MessageConsumerEvidence {
            isa_profile: r5900::PROFILE_ID,
            consumer_symbol: spec.consumer_symbol,
            consumer_address: format!("{consumer_address:#010x}"),
            consumer_byte_length: consumer_bytes.len(),
            consumer_sha256: sha256(&consumer_bytes),
            consumer_instruction_count: consumer_disassembly.instructions.len(),
            renderer_symbol: RENDERER_SYMBOL,
            renderer_address: format!("{renderer_address:#010x}"),
            renderer_byte_length: renderer_bytes.len(),
            renderer_sha256: sha256(&renderer_bytes),
            renderer_instruction_count: renderer_disassembly.instructions.len(),
            nonnull_message_store_count: nonnull_stores.len(),
            ignored_null_message_stores: null_stores,
            control_dispatch: validate_control_dispatch(&renderer_bytes, renderer_address)?,
        },
        control_codes: controls,
        references,
        messages,
        translation_scope: MessageTranslationScope {
            target: "non-ASCII Shift-JIS runs in Japanese messages",
            preserve: "English and other ASCII runs by default, plus all runtime controls",
            report_role: "read-only corpus evidence; not an approved translation asset",
        },
    })
}

pub(super) fn register_message_reference(
    messages: &mut BTreeMap<&str, (MessageSpec, Vec<String>)>,
    spec: MessageSpec,
    reference_id: &str,
) -> Result<()> {
    let entry = messages
        .entry(spec.symbol)
        .or_insert_with(|| (spec, Vec::new()));
    ensure!(
        entry.0.id == spec.id,
        "one message symbol has conflicting stable IDs: {}",
        spec.symbol
    );
    entry.1.push(reference_id.to_string());
    Ok(())
}

pub(super) fn find_renderer_actor_creation_sites(
    file: &object::File<'_>,
    renderer_address: u64,
    setter_address: u64,
) -> Result<Vec<RendererActorCreationSite>> {
    let mut sites = Vec::new();
    for symbol in file.symbols() {
        if !symbol.is_definition() || symbol.kind() != SymbolKind::Text || symbol.size() == 0 {
            continue;
        }
        let name = symbol
            .name()
            .context("ELF function symbol has no valid name")?;
        let section_index = symbol
            .section_index()
            .with_context(|| format!("ELF function symbol has no section: {name}"))?;
        let section = file.section_by_index(section_index)?;
        let section_data = section.data()?;
        let relative = usize::try_from(
            symbol
                .address()
                .checked_sub(section.address())
                .with_context(|| format!("ELF function precedes its section: {name}"))?,
        )?;
        let byte_length = usize::try_from(symbol.size())?;
        let source = section_data
            .get(relative..relative + byte_length)
            .with_context(|| format!("ELF function bytes exceed section: {name}"))?;
        for actor in
            find_actor_creation_specs(source, symbol.address(), renderer_address, setter_address)?
        {
            sites.push(RendererActorCreationSite {
                producer_symbol: name.to_string(),
                producer_address: symbol.address(),
                actor,
            });
        }
    }
    sites.sort_by_key(|site| (site.producer_address, site.actor.upper_offset));
    Ok(sites)
}

pub(super) fn find_actor_creation_specs(
    source: &[u8],
    start_address: u64,
    renderer_address: u64,
    setter_address: u64,
) -> Result<Vec<ActorCreationSpec>> {
    let renderer_address = u32::try_from(renderer_address)
        .context("renderer address exceeds the EE code address space")?;
    let expected_upper = renderer_address.wrapping_add(0x8000) >> 16;
    let expected_lower = renderer_address as u16;
    let mut actors = Vec::new();

    for upper_offset in (0..source.len().saturating_sub(3)).step_by(4) {
        let Some(upper_word) = instruction_word(source, upper_offset) else {
            continue;
        };
        if upper_word >> 26 != 0x0f || upper_word & 0xffff != expected_upper {
            continue;
        }
        let upper = decode_instruction(source, start_address, upper_offset)?;
        if !matches!(
            upper.operands(),
            Operands::LoadUpper { destination, immediate }
                if *destination == GeneralRegister::A0
                    && u32::from(*immediate) == expected_upper
        ) {
            continue;
        }

        let mut lower_offsets = Vec::new();
        let mut call_offsets = Vec::new();
        for delta in [4_usize, 8, 12, 16, 20] {
            let candidate_offset = upper_offset + delta;
            let Some(candidate_word) = instruction_word(source, candidate_offset) else {
                continue;
            };
            if candidate_word >> 26 == 0x09 && candidate_word as u16 == expected_lower {
                let instruction = decode_instruction(source, start_address, candidate_offset)?;
                if matches!(
                    instruction.operands(),
                    Operands::SignedImmediate { destination, source, immediate }
                        if *destination == GeneralRegister::A0
                            && *source == GeneralRegister::A0
                            && *immediate as u16 == expected_lower
                ) {
                    lower_offsets.push(candidate_offset);
                }
            }
            if candidate_word >> 26 == 0x03 {
                let instruction = decode_instruction(source, start_address, candidate_offset)?;
                if matches!(
                    instruction.operands(),
                    Operands::Jump { target } if u64::from(*target) == setter_address
                ) {
                    call_offsets.push(candidate_offset);
                }
            }
        }
        ensure!(
            lower_offsets.len() <= 1 && call_offsets.len() <= 1,
            "renderer actor creation sequence is ambiguous"
        );
        if let ([lower_offset], [call_offset]) = (lower_offsets.as_slice(), call_offsets.as_slice())
        {
            actors.push(ActorCreationSpec {
                upper_offset,
                lower_offset: *lower_offset,
                call_offset: *call_offset,
            });
        }
    }
    Ok(actors)
}
