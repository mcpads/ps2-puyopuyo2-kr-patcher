use super::*;

pub(super) fn validate_rank_slot_one_registration(file: &object::File<'_>) -> Result<()> {
    let (_, bytes) = read_symbol_bytes(file, "RankTexList")?;
    let pairs = parse_texture_registration_pairs("RankTexList", &bytes)?;
    ensure!(
        pairs.len() == 7
            && pairs.get(G122_REGISTRATION_SLOT)
                == Some(&TextureRegistrationPair {
                    image_load_index: 5,
                    palette_slot: 5,
                }),
        "RankTexList no longer maps RegBuff slot 1 to RankTexTbl image 5/palette 5"
    );
    Ok(())
}

pub(super) fn validate_direct_pattern_call_census(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<()> {
    let setter = read_executable_symbol(executable, "Act_SetPtnDirect")?;
    let calls = find_direct_calls(
        file,
        &BTreeMap::from([(
            u32::try_from(setter.address)?,
            "Act_SetPtnDirect".to_string(),
        )]),
    )?;
    let actual = calls
        .iter()
        .map(|call| parse_hex_address(&call.address))
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        calls.len() == DIRECT_PATTERN_CALL_ADDRESSES.len()
            && actual == DIRECT_PATTERN_CALL_ADDRESSES.into_iter().collect(),
        "Act_SetPtnDirect call population changed from the audited 27-site census"
    );
    let expected_relocations = DIRECT_PATTERN_CALL_ADDRESSES
        .into_iter()
        .map(|address| (address, object::elf::R_MIPS_26))
        .collect::<Vec<_>>();
    let relocation_sites =
        validate_main_symbol_relocations(file, "Act_SetPtnDirect", &expected_relocations)?;
    ensure!(
        relocation_sites
            .iter()
            .map(|(address, _)| *address)
            .collect::<BTreeSet<_>>()
            == actual,
        "raw Act_SetPtnDirect calls and .relmain R_MIPS_26 population differ"
    );
    let mut slots = BTreeMap::<i16, usize>::new();
    let mut dynamic_slot_one = BTreeSet::new();
    for call in &calls {
        let address = parse_hex_address(&call.address)?;
        let slot = resolve_direct_pattern_slot(executable, call)?;
        *slots.entry(slot).or_default() += 1;
        if slot == 1 {
            ensure!(
                call.owner_symbols
                    .iter()
                    .any(|owner| owner == "FontPrint_Act"),
                "a non-FontPrint_Act direct-pattern call selects RegBuff slot 1"
            );
            dynamic_slot_one.insert(address);
        }
    }
    ensure!(
        slots == BTreeMap::from([(0, 2), (1, 2), (2, 15), (3, 4), (4, 3), (9, 1)]),
        "Act_SetPtnDirect resolved-slot multiset changed"
    );
    ensure!(
        dynamic_slot_one
            == DYNAMIC_SLOT_ONE_PATTERN_CALL_ADDRESSES
                .into_iter()
                .collect(),
        "the audited slot-1 direct-pattern consumers are no longer exactly the two FontPrint_Act calls"
    );
    Ok(())
}

fn resolve_direct_pattern_slot(
    executable: &[u8],
    call: &crate::executable_references::DirectCallReference,
) -> Result<i16> {
    let call_address = parse_hex_address(&call.address)?;
    let (owner_name, owner) = call
        .owner_symbols
        .iter()
        .filter_map(|name| {
            let symbol = read_executable_symbol(executable, name).ok()?;
            (call_address >= symbol.address
                && call_address < symbol.address + u64::try_from(symbol.bytes.len()).ok()?)
            .then_some((name.as_str(), symbol))
        })
        .min_by_key(|(_, symbol)| symbol.bytes.len())
        .with_context(|| format!("no text owner contains direct-pattern call {call_address:#x}"))?;
    if owner_name == "FontPrint_Act" {
        validate_fontprint_unique_slot_writer(&owner.bytes, owner.address)?;
        validate_no_external_control_entry(
            &owner.bytes,
            owner.address,
            4,
            usize::try_from(call_address - owner.address)?,
            owner_name,
        )?;
        let instruction = decode_instruction(&owner.bytes, owner.address, 4)?;
        return immediate_a1_value(&instruction).context("FontPrint_Act slot setup changed");
    }

    let call_offset = usize::try_from(call_address - owner.address)?;
    let delay_offset = call_offset
        .checked_add(4)
        .context("direct-pattern delay-slot offset overflow")?;
    let mut offsets = vec![delay_offset];
    offsets.extend((0..call_offset).step_by(4).rev());
    for offset in offsets {
        let instruction = decode_instruction(&owner.bytes, owner.address, offset)?;
        if instruction_writes_a1(&instruction, owner.address + u64::try_from(offset)?)? {
            ensure!(
                delay_offset.abs_diff(offset) <= 0x40,
                "{owner_name} direct-pattern slot definition is not local to call {call_address:#x}"
            );
            validate_straight_line_slot_lineage(
                &owner.bytes,
                owner.address,
                offset,
                call_offset,
                delay_offset,
                owner_name,
            )?;
            return immediate_a1_value(&instruction).with_context(|| {
                format!(
                    "{owner_name} direct-pattern slot at {call_address:#x} is not an immediate A1 value"
                )
            });
        }
        ensure!(
            offset == delay_offset || instruction.form() != InstructionForm::Jal,
            "{owner_name} direct-pattern slot at {call_address:#x} crosses an intervening call"
        );
    }
    anyhow::bail!("{owner_name} direct-pattern call {call_address:#x} has no A1 definition")
}

fn validate_fontprint_unique_slot_writer(source: &[u8], start_address: u64) -> Result<()> {
    let mut writers = Vec::new();
    for offset in (0..source.len()).step_by(4) {
        let instruction = decode_instruction(source, start_address, offset)?;
        if instruction_writes_a1(&instruction, start_address + u64::try_from(offset)?)? {
            writers.push(offset);
        }
    }
    ensure!(
        writers == [4],
        "FontPrint_Act no longer has exactly one A1 writer at entry offset 4"
    );
    Ok(())
}

fn validate_straight_line_slot_lineage(
    source: &[u8],
    start_address: u64,
    definition_offset: usize,
    call_offset: usize,
    delay_offset: usize,
    owner_name: &str,
) -> Result<()> {
    let mut offsets = if definition_offset < call_offset {
        ((definition_offset + 4)..call_offset)
            .step_by(4)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if definition_offset != delay_offset {
        offsets.push(delay_offset);
    }
    for offset in offsets {
        let instruction = decode_instruction(source, start_address, offset)?;
        let semantics = r5900::R5900::semantics(
            &instruction,
            &r5900::AnalysisPoint::new(r5900::CodeLocation::try_from(
                start_address + u64::try_from(offset)?,
            )?),
        )?;
        ensure!(
            semantics.control_flow.action == ControlAction::Continue,
            "{owner_name} direct-pattern A1 lineage crosses control flow before call at {:#x}",
            start_address + u64::try_from(call_offset)?
        );
    }
    validate_no_external_control_entry(
        source,
        start_address,
        definition_offset,
        call_offset,
        owner_name,
    )?;
    Ok(())
}

fn validate_no_external_control_entry(
    source: &[u8],
    start_address: u64,
    definition_offset: usize,
    call_offset: usize,
    owner_name: &str,
) -> Result<()> {
    let definition_address = start_address + u64::try_from(definition_offset)?;
    let call_address = start_address + u64::try_from(call_offset)?;
    for offset in (0..source.len()).step_by(4) {
        let source_address = start_address + u64::try_from(offset)?;
        let instruction = decode_instruction(source, start_address, offset)?;
        let semantics = r5900::R5900::semantics(
            &instruction,
            &r5900::AnalysisPoint::new(r5900::CodeLocation::try_from(source_address)?),
        )?;
        let target = match semantics.control_flow.action {
            ControlAction::Transfer {
                target: ControlTarget::Direct(target),
            }
            | ControlAction::LinkedTransfer {
                target: ControlTarget::Direct(target),
                ..
            } => u64::from(target.location),
            _ => continue,
        };
        let source_is_after_definition =
            source_address >= definition_address && source_address < call_address;
        ensure!(
            target <= definition_address || target > call_address || source_is_after_definition,
            "{owner_name} direct-pattern A1 lineage has an external control edge from {source_address:#x} into {target:#x}"
        );
    }
    Ok(())
}

fn immediate_a1_value(instruction: &r5900::Instruction) -> Option<i16> {
    match instruction.operands() {
        Operands::SignedImmediate {
            destination: GeneralRegister::A1,
            source: GeneralRegister::ZERO,
            immediate,
        } if matches!(
            instruction.form(),
            InstructionForm::Addiu | InstructionForm::Daddiu
        ) =>
        {
            Some(*immediate)
        }
        Operands::ThreeGeneral {
            destination: GeneralRegister::A1,
            left: GeneralRegister::ZERO,
            right: GeneralRegister::ZERO,
        } if matches!(
            instruction.form(),
            InstructionForm::Addu | InstructionForm::Daddu | InstructionForm::Or
        ) =>
        {
            Some(0)
        }
        _ => None,
    }
}

fn instruction_writes_a1(instruction: &r5900::Instruction, address: u64) -> Result<bool> {
    let semantics = r5900::R5900::semantics(
        instruction,
        &r5900::AnalysisPoint::new(r5900::CodeLocation::try_from(address)?),
    )?;
    Ok(semantics.location_accesses.iter().any(|access| {
        access.kind == AccessKind::Write
            && matches!(
                access.location,
                Location::General(storage) if storage.register == GeneralRegister::A1
            )
    }))
}

pub(super) fn validate_unreachable_game_pattern_slot_one(file: &object::File<'_>) -> Result<()> {
    let (address, bytes) = read_symbol_bytes(file, "GamePtn")?;
    let records = parse_pattern_records("GamePtn", &bytes)?;
    ensure!(records.len() == 164, "GamePtn population changed");
    let slot_one_records = records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            (record.image_load_index == G122_REGISTRATION_SLOT).then_some(index)
        })
        .collect::<Vec<_>>();
    ensure!(
        slot_one_records == [163]
            && records[0] == pattern(0, 0, 448, 640, 448)
            && records[1] == pattern(0, 0, 0, 640, 448)
            && records[163] == pattern(1, 0, 0, 640, 448),
        "audited GamePtn live/dead records changed"
    );
    validate_main_symbol_relocations(file, "GamePtn", &GAME_PATTERN_RELOCATIONS)?;
    let targets = BTreeMap::from([
        (u32::try_from(address)?, "GamePtn[0]".to_string()),
        (u32::try_from(address + 24)?, "GamePtn[1]".to_string()),
        (
            u32::try_from(address + 163 * 24)?,
            "GamePtn[163]".to_string(),
        ),
    ]);
    let references = find_static_references(file, &targets)?;
    let expected_live = [
        ("GamePtn[0]", "Dbg_Face_Run", "0x0045ca68"),
        ("GamePtn[1]", "Dbg_Face_Run", "0x0045ca80"),
    ];
    for (target, owner, reference_address) in expected_live {
        let found = references
            .get(target)
            .map(Vec::as_slice)
            .unwrap_or_default();
        ensure!(
            found.len() == 1
                && found[0].owner_symbol == owner
                && found[0].address == reference_address,
            "{target} live reference population changed"
        );
    }
    validate_dbg_face_game_pattern_consumers(file, address)?;
    ensure!(
        references.get("GamePtn[163]").is_none_or(Vec::is_empty),
        "dead GamePtn slot-1 full-rectangle record gained a static reference"
    );
    Ok(())
}

fn validate_dbg_face_game_pattern_consumers(
    file: &object::File<'_>,
    game_pattern: u64,
) -> Result<()> {
    let (owner_address, owner_bytes) = read_symbol_bytes(file, "Dbg_Face_Run")?;
    let (consumer_address, _) = read_symbol_bytes(file, "Make_DispOnly")?;
    for (upper_offset, lower_offset, call_offset, expected_record) in [
        (0x158, 0x16c, 0x168, game_pattern),
        (0x170, 0x184, 0x180, game_pattern + 24),
    ] {
        validate_loaded_address(
            &owner_bytes,
            owner_address,
            upper_offset,
            lower_offset,
            GeneralRegister::A3,
            expected_record,
            "Dbg_Face fixed GamePtn record",
        )?;
        validate_jump_target(
            &owner_bytes,
            owner_address,
            call_offset,
            consumer_address,
            "Dbg_Face fixed GamePtn consumer",
        )?;
    }
    Ok(())
}

fn pattern(
    registration_slot: usize,
    source_x: u32,
    source_y: u32,
    width: u32,
    height: u32,
) -> TexturePatternRecord {
    TexturePatternRecord {
        image_load_index: registration_slot,
        source_x,
        source_y,
        width,
        height,
    }
}

fn parse_hex_address(source: &str) -> Result<u64> {
    u64::from_str_radix(source.trim_start_matches("0x"), 16)
        .with_context(|| format!("invalid direct-call address: {source}"))
}

pub(super) fn validate_no_slot_one_patterns(
    symbol: &str,
    records: &[TexturePatternRecord],
) -> Result<()> {
    ensure!(
        records
            .iter()
            .all(|record| record.image_load_index != G122_REGISTRATION_SLOT),
        "{symbol} has a fixed pattern that consumes RegBuff slot 1; G122 glyph reassignment is unsafe"
    );
    Ok(())
}

#[cfg(test)]
#[path = "pattern_usage_tests.rs"]
mod pattern_usage_tests;
