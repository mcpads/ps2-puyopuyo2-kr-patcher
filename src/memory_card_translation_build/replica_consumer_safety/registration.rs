use super::*;

pub(super) fn validate_frame_and_nazo_end_slot_one_registration(
    file: &object::File<'_>,
) -> Result<()> {
    validate_regist2_sequential_image_slot_layout(file)?;
    validate_palette_cursor_layout(file)?;
    let frame_tables = validate_frame_selection_tables(file)?;
    validate_direct_regist2_after_reset(
        file,
        "Puyo_Toko_Run",
        0x50,
        "FrameTexTbl_05",
        0x58,
        0x64,
        0x60,
    )?;
    validate_direct_regist2_after_reset(
        file,
        "Dbg_Face_Run",
        0x50,
        "FrameTexTbl_00",
        0x58,
        0x64,
        0x60,
    )?;
    validate_direct_regist2_after_reset(
        file,
        "Nazo_Ending_Run",
        0x78,
        "NazoEndTexTbl",
        0x80,
        0x8c,
        0x88,
    )?;
    validate_main_symbol_relocations(
        file,
        "NazoEndTexTbl",
        &[
            (0x0045_ea80, object::elf::R_MIPS_HI16),
            (0x0045_ea8c, object::elf::R_MIPS_LO16),
            (0x0045_ebd4, object::elf::R_MIPS_HI16),
            (0x0045_ebe4, object::elf::R_MIPS_LO16),
        ],
    )?;

    validate_selection_regist2_after_reset(
        file,
        "Play_Demo_Run",
        0x78,
        "FrameTexTbl_1P_T",
        0x84,
        0x8c,
        0x94,
        0x98,
        0x9c,
    )?;
    validate_frame_selection_storage(file)?;
    for (owner, init_call, table_load, registrar_call) in [
        ("Puyo_1P_Run", 0xe8, 0xf0, 0xf4),
        ("Puyo_2P_Run", 0xd0, 0xd8, 0xdc),
        ("Puyo_2P_Run", 0x548, 0x550, 0x554),
    ] {
        validate_stored_selection_regist2_after_reset(
            file,
            owner,
            init_call,
            table_load,
            registrar_call,
        )?;
    }
    validate_frame_table_relocation_population(file, &frame_tables)?;
    Ok(())
}

fn validate_regist2_sequential_image_slot_layout(file: &object::File<'_>) -> Result<()> {
    let buffer_address = symbol_address(file, "RegBuff")?;
    let (init_address, init_bytes) = read_symbol_bytes(file, "ActRegList_Init")?;
    ensure!(
        init_bytes.len() == 56,
        "ActRegList_Init instruction population changed"
    );
    validate_loaded_address(
        &init_bytes,
        init_address,
        0x04,
        0x0c,
        GeneralRegister::A0,
        buffer_address,
        "Regist2 registration-buffer reset",
    )?;
    require_three_general(
        &init_bytes,
        init_address,
        0x10,
        InstructionForm::Daddu,
        GeneralRegister::A1,
        GeneralRegister::ZERO,
        GeneralRegister::ZERO,
        "Regist2 registration-buffer zero fill",
    )?;
    require_memory(
        &init_bytes,
        init_address,
        0x1c,
        InstructionForm::Sw,
        GeneralRegister::ZERO,
        GeneralRegister::GP,
        -0x7784,
        "Regist2 registration-slot counter reset",
    )?;
    require_signed_immediate(
        &init_bytes,
        init_address,
        0x20,
        InstructionForm::Addiu,
        GeneralRegister::V1,
        GeneralRegister::ZERO,
        0x1a40,
        "Regist2 palette cursor initial value",
    )?;
    require_memory(
        &init_bytes,
        init_address,
        0x28,
        InstructionForm::Sw,
        GeneralRegister::V1,
        GeneralRegister::GP,
        -0x778c,
        "Regist2 palette cursor reset",
    )?;

    let (registrar_address, registrar_bytes) = read_symbol_bytes(file, "ActRegList_Regist2")?;
    ensure!(
        registrar_bytes.len() == 552,
        "ActRegList_Regist2 instruction population changed"
    );
    require_three_general(
        &registrar_bytes,
        registrar_address,
        0x18,
        InstructionForm::Daddu,
        GeneralRegister::S5,
        GeneralRegister::A1,
        GeneralRegister::ZERO,
        "Regist2 load-table cursor",
    )?;
    require_memory(
        &registrar_bytes,
        registrar_address,
        0x48,
        InstructionForm::Lw,
        GeneralRegister::A0,
        GeneralRegister::S5,
        4,
        "Regist2 image-operation discriminator",
    )?;
    require_signed_immediate(
        &registrar_bytes,
        registrar_address,
        0x4c,
        InstructionForm::Addiu,
        GeneralRegister::V1,
        GeneralRegister::ZERO,
        1,
        "Regist2 image-operation value",
    )?;
    let image_branch = decode_instruction(&registrar_bytes, registrar_address, 0x50)?;
    ensure!(
        image_branch.form() == InstructionForm::Bne
            && matches!(
                image_branch.operands(),
                Operands::BranchTwo {
                    left: GeneralRegister::A0,
                    right: GeneralRegister::V1,
                    target: 0x0043_3580,
                }
            ),
        "Regist2 no longer registers only the contiguous image prefix"
    );
    require_memory(
        &registrar_bytes,
        registrar_address,
        0x58,
        InstructionForm::Lw,
        GeneralRegister::A1,
        GeneralRegister::GP,
        -0x7784,
        "Regist2 registration-slot counter load",
    )?;
    validate_loaded_address(
        &registrar_bytes,
        registrar_address,
        0x5c,
        0x60,
        GeneralRegister::V1,
        buffer_address,
        "Regist2 registration-buffer base",
    )?;
    require_shift(
        &registrar_bytes,
        registrar_address,
        0x64,
        InstructionForm::Sll,
        GeneralRegister::A0,
        GeneralRegister::A1,
        3,
        "Regist2 slot index times eight",
    )?;
    require_signed_immediate(
        &registrar_bytes,
        registrar_address,
        0x68,
        InstructionForm::Addiu,
        GeneralRegister::A2,
        GeneralRegister::A1,
        1,
        "Regist2 next registration-slot index",
    )?;
    require_three_general(
        &registrar_bytes,
        registrar_address,
        0x6c,
        InstructionForm::Addu,
        GeneralRegister::A0,
        GeneralRegister::A0,
        GeneralRegister::A1,
        "Regist2 slot index times nine",
    )?;
    require_shift(
        &registrar_bytes,
        registrar_address,
        0x74,
        InstructionForm::Sll,
        GeneralRegister::A0,
        GeneralRegister::A0,
        4,
        "Regist2 0x90-byte registration stride",
    )?;
    require_three_general(
        &registrar_bytes,
        registrar_address,
        0x7c,
        InstructionForm::Addu,
        GeneralRegister::S4,
        GeneralRegister::V1,
        GeneralRegister::A0,
        "Regist2 registration-record address",
    )?;
    require_memory(
        &registrar_bytes,
        registrar_address,
        0x94,
        InstructionForm::Sw,
        GeneralRegister::A2,
        GeneralRegister::GP,
        -0x7784,
        "Regist2 registration-slot counter store",
    )?;
    require_memory(
        &registrar_bytes,
        registrar_address,
        0xa4,
        InstructionForm::Lw,
        GeneralRegister::A0,
        GeneralRegister::GP,
        -0x778c,
        "Regist2 current palette cursor",
    )?;
    require_memory(
        &registrar_bytes,
        registrar_address,
        0xa8,
        InstructionForm::Sw,
        GeneralRegister::A0,
        GeneralRegister::S4,
        4,
        "Regist2 slot palette address",
    )?;
    require_signed_immediate(
        &registrar_bytes,
        registrar_address,
        0x1d0,
        InstructionForm::Addiu,
        GeneralRegister::S5,
        GeneralRegister::S5,
        0x10,
        "Regist2 next load-table record",
    )?;
    require_signed_immediate(
        &registrar_bytes,
        registrar_address,
        0x1d8,
        InstructionForm::Addiu,
        GeneralRegister::V1,
        GeneralRegister::ZERO,
        -1,
        "Regist2 load-table terminator",
    )?;
    require_signed_immediate(
        &registrar_bytes,
        registrar_address,
        0x1e4,
        InstructionForm::Addiu,
        GeneralRegister::A0,
        GeneralRegister::A0,
        4,
        "Regist2 next palette cursor",
    )?;
    require_memory(
        &registrar_bytes,
        registrar_address,
        0x1e8,
        InstructionForm::Sw,
        GeneralRegister::A0,
        GeneralRegister::GP,
        -0x778c,
        "Regist2 palette cursor store",
    )?;
    let loop_branch = decode_instruction(&registrar_bytes, registrar_address, 0x1f8)?;
    ensure!(
        loop_branch.form() == InstructionForm::Bne
            && matches!(
                loop_branch.operands(),
                Operands::BranchTwo {
                    left: GeneralRegister::A0,
                    right: GeneralRegister::V1,
                    target: 0x0043_33c8,
                }
            ),
        "Regist2 image-record loop changed"
    );
    Ok(())
}

fn validate_palette_cursor_layout(file: &object::File<'_>) -> Result<()> {
    const DISPATCH_TABLE_ADDRESS: u64 = 0x004e_edd0;
    const DISPATCH_TARGETS: [u32; 6] = [
        0x0042_4018,
        0x0042_3f98,
        0x0042_3fe8,
        0x0042_3f48,
        0x0042_4078,
        0x0042_4078,
    ];

    let main = file
        .section_by_name("main")
        .context("ELF has no main section for texture-loader validation")?;
    let main_data = main.data()?;
    let table_offset = usize::try_from(
        DISPATCH_TABLE_ADDRESS
            .checked_sub(main.address())
            .context("texture-loader dispatch table precedes main")?,
    )?;
    let table_bytes = main_data
        .get(table_offset..table_offset + DISPATCH_TARGETS.len() * 4)
        .context("texture-loader dispatch table leaves main")?;
    let targets = table_bytes
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four-byte chunk")))
        .collect::<Vec<_>>();
    ensure!(
        targets == DISPATCH_TARGETS,
        "aqCdvd_ReadFile operation dispatch table changed"
    );
    let dispatch_relocations = (0..DISPATCH_TARGETS.len())
        .map(|index| {
            Ok((
                DISPATCH_TABLE_ADDRESS + u64::try_from(index * 4)?,
                object::elf::R_MIPS_32,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    validate_main_symbol_relocations_of_type(
        file,
        "aqCdvd_ReadFile",
        object::elf::R_MIPS_32,
        &dispatch_relocations,
    )?;

    let (loader_address, loader_bytes) = read_symbol_bytes(file, "aqCdvd_ReadFile")?;
    ensure!(
        loader_bytes.len() == 0x23c,
        "aqCdvd_ReadFile instruction population changed"
    );
    require_three_general(
        &loader_bytes,
        loader_address,
        0x60,
        InstructionForm::Daddu,
        GeneralRegister::S2,
        GeneralRegister::ZERO,
        GeneralRegister::ZERO,
        "texture-loader implicit palette cursor reset",
    )?;
    require_memory(
        &loader_bytes,
        loader_address,
        0xa0,
        InstructionForm::Lw,
        GeneralRegister::A0,
        GeneralRegister::S1,
        4,
        "texture-loader operation field",
    )?;
    require_shift(
        &loader_bytes,
        loader_address,
        0xb0,
        InstructionForm::Sll,
        GeneralRegister::A0,
        GeneralRegister::A0,
        2,
        "texture-loader operation dispatch index",
    )?;
    validate_loaded_address(
        &loader_bytes,
        loader_address,
        0xac,
        0xb4,
        GeneralRegister::A1,
        DISPATCH_TABLE_ADDRESS,
        "texture-loader operation dispatch table",
    )?;
    require_three_general(
        &loader_bytes,
        loader_address,
        0xb8,
        InstructionForm::Addu,
        GeneralRegister::A0,
        GeneralRegister::A0,
        GeneralRegister::A1,
        "texture-loader operation dispatch entry",
    )?;
    require_memory(
        &loader_bytes,
        loader_address,
        0xbc,
        InstructionForm::Lw,
        GeneralRegister::A0,
        GeneralRegister::A0,
        0,
        "texture-loader operation dispatch target",
    )?;
    ensure!(
        decode_instruction(&loader_bytes, loader_address, 0xc0)?.form() == InstructionForm::Jr,
        "texture-loader operation dispatch transfer changed"
    );

    require_memory(
        &loader_bytes,
        loader_address,
        0x168,
        InstructionForm::Lw,
        GeneralRegister::A0,
        GeneralRegister::S1,
        0,
        "palette-load flags",
    )?;
    require_unsigned_immediate(
        &loader_bytes,
        loader_address,
        0x16c,
        InstructionForm::Andi,
        GeneralRegister::A0,
        GeneralRegister::A0,
        1,
        "palette-load explicit-destination flag",
    )?;
    let destination_branch = decode_instruction(&loader_bytes, loader_address, 0x170)?;
    ensure!(
        destination_branch.form() == InstructionForm::Beq
            && matches!(
                destination_branch.operands(),
                Operands::BranchTwo {
                    left: GeneralRegister::A0,
                    right: GeneralRegister::ZERO,
                    target: 0x0042_4008,
                }
            ),
        "palette-load explicit/implicit destination branch changed"
    );
    require_memory(
        &loader_bytes,
        loader_address,
        0x178,
        InstructionForm::Lw,
        GeneralRegister::S2,
        GeneralRegister::S1,
        0xc,
        "palette-load explicit destination",
    )?;
    require_signed_immediate(
        &loader_bytes,
        loader_address,
        0x180,
        InstructionForm::Addiu,
        GeneralRegister::S2,
        GeneralRegister::S2,
        4,
        "palette-load cursor after explicit destination",
    )?;
    require_memory(
        &loader_bytes,
        loader_address,
        0x188,
        InstructionForm::Sw,
        GeneralRegister::S2,
        GeneralRegister::S1,
        0xc,
        "palette-load implicit destination",
    )?;
    require_signed_immediate(
        &loader_bytes,
        loader_address,
        0x190,
        InstructionForm::Addiu,
        GeneralRegister::S2,
        GeneralRegister::S2,
        4,
        "palette-load cursor after implicit destination",
    )?;
    Ok(())
}

fn validate_frame_selection_tables(file: &object::File<'_>) -> Result<BTreeMap<String, u64>> {
    let frame_tables = OWNER_LOAD_TABLES[..11]
        .iter()
        .map(|owner| {
            let (address, _) = read_symbol_bytes(file, owner.symbol)?;
            Ok((owner.symbol.to_string(), address))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let allowed = frame_tables.values().copied().collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    for (symbol, expected_records) in FRAME_SELECTION_TABLES {
        let (_, bytes) = read_symbol_bytes(file, symbol)?;
        ensure!(
            bytes.len() == expected_records * 4,
            "{symbol} frame-selection population changed"
        );
        for pointer in bytes.chunks_exact(4) {
            let address = u64::from(u32::from_le_bytes(pointer.try_into()?));
            ensure!(
                allowed.contains(&address),
                "{symbol} selects an unvalidated frame load table at {address:#x}"
            );
            selected.insert(address);
        }
    }
    ensure!(
        selected == allowed,
        "frame-selection tables no longer cover exactly the 11 audited G122 frame owners"
    );
    Ok(frame_tables)
}

#[allow(clippy::too_many_arguments)]
fn validate_direct_regist2_after_reset(
    file: &object::File<'_>,
    owner_symbol: &str,
    init_call_offset: usize,
    table_symbol: &str,
    table_upper_offset: usize,
    table_lower_offset: usize,
    registrar_call_offset: usize,
) -> Result<()> {
    let (owner_address, owner_bytes) = read_symbol_bytes(file, owner_symbol)?;
    let (init_address, _) = read_symbol_bytes(file, "ActRegList_Init")?;
    let (registrar_address, _) = read_symbol_bytes(file, "ActRegList_Regist2")?;
    let (table_address, _) = read_symbol_bytes(file, table_symbol)?;
    validate_jump_target(
        &owner_bytes,
        owner_address,
        init_call_offset,
        init_address,
        "Regist2 slot reset before owner registration",
    )?;
    validate_loaded_address(
        &owner_bytes,
        owner_address,
        table_upper_offset,
        table_lower_offset,
        GeneralRegister::A1,
        table_address,
        "Regist2 owner load table",
    )?;
    validate_jump_target(
        &owner_bytes,
        owner_address,
        registrar_call_offset,
        registrar_address,
        "Regist2 owner registration",
    )?;
    validate_regist2_image_file_id(
        &owner_bytes,
        owner_address,
        registrar_call_offset,
        owner_symbol,
    )?;
    validate_control_continuity_between_registration_calls(
        &owner_bytes,
        owner_address,
        init_call_offset,
        registrar_call_offset,
        owner_symbol,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_selection_regist2_after_reset(
    file: &object::File<'_>,
    owner_symbol: &str,
    init_call_offset: usize,
    selection_symbol: &str,
    selection_upper_offset: usize,
    selection_lower_offset: usize,
    selection_add_offset: usize,
    pointer_load_offset: usize,
    registrar_call_offset: usize,
) -> Result<()> {
    let (owner_address, owner_bytes) = read_symbol_bytes(file, owner_symbol)?;
    let (selection_address, _) = read_symbol_bytes(file, selection_symbol)?;
    let (init_address, _) = read_symbol_bytes(file, "ActRegList_Init")?;
    let (registrar_address, _) = read_symbol_bytes(file, "ActRegList_Regist2")?;
    validate_jump_target(
        &owner_bytes,
        owner_address,
        init_call_offset,
        init_address,
        "frame-selection Regist2 slot reset",
    )?;
    validate_loaded_address(
        &owner_bytes,
        owner_address,
        selection_upper_offset,
        selection_lower_offset,
        GeneralRegister::V0,
        selection_address,
        "frame-selection table base",
    )?;
    require_three_general(
        &owner_bytes,
        owner_address,
        selection_add_offset,
        InstructionForm::Addu,
        GeneralRegister::V0,
        GeneralRegister::V0,
        GeneralRegister::V1,
        "frame-selection pointer address",
    )?;
    require_memory(
        &owner_bytes,
        owner_address,
        pointer_load_offset,
        InstructionForm::Lw,
        GeneralRegister::A1,
        GeneralRegister::V0,
        0,
        "frame-selection load table pointer",
    )?;
    validate_jump_target(
        &owner_bytes,
        owner_address,
        registrar_call_offset,
        registrar_address,
        "selected frame Regist2 registration",
    )?;
    validate_regist2_image_file_id(
        &owner_bytes,
        owner_address,
        registrar_call_offset,
        owner_symbol,
    )?;
    validate_control_continuity_between_registration_calls(
        &owner_bytes,
        owner_address,
        init_call_offset,
        registrar_call_offset,
        owner_symbol,
    )
}

fn validate_control_continuity_between_registration_calls(
    source: &[u8],
    start_address: u64,
    init_call_offset: usize,
    registrar_call_offset: usize,
    owner_symbol: &str,
) -> Result<()> {
    for offset in ((init_call_offset + 4)..registrar_call_offset).step_by(4) {
        let instruction = decode_instruction(source, start_address, offset)?;
        let semantics = r5900::R5900::semantics(
            &instruction,
            &r5900::AnalysisPoint::new(r5900::CodeLocation::try_from(
                start_address + u64::try_from(offset)?,
            )?),
        )?;
        ensure!(
            semantics.control_flow.action == ControlAction::Continue,
            "{owner_symbol} branches between ActRegList_Init and its first Regist2 call"
        );
    }
    let delay_offset = registrar_call_offset + 4;
    let delay = decode_instruction(source, start_address, delay_offset)?;
    let semantics = r5900::R5900::semantics(
        &delay,
        &r5900::AnalysisPoint::new(r5900::CodeLocation::try_from(
            start_address + u64::try_from(delay_offset)?,
        )?),
    )?;
    ensure!(
        semantics.control_flow.action == ControlAction::Continue,
        "{owner_symbol} Regist2 delay slot changed control flow"
    );
    Ok(())
}

fn validate_regist2_image_file_id(
    source: &[u8],
    start_address: u64,
    registrar_call_offset: usize,
    owner_symbol: &str,
) -> Result<()> {
    let before_call = registrar_call_offset
        .checked_sub(4)
        .context("Regist2 call has no preceding instruction")?;
    let delay_slot = registrar_call_offset
        .checked_add(4)
        .context("Regist2 delay-slot offset overflow")?;
    let image_g_selector = [before_call, delay_slot]
        .into_iter()
        .map(|offset| decode_instruction(source, start_address, offset))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .any(|instruction| {
            matches!(
                instruction.operands(),
                Operands::SignedImmediate {
                    destination: GeneralRegister::A0,
                    source: GeneralRegister::ZERO,
                    immediate: 1,
                }
            ) && matches!(
                instruction.form(),
                InstructionForm::Addiu | InstructionForm::Daddiu
            )
        });
    ensure!(
        image_g_selector,
        "{owner_symbol} no longer passes IMAGE_G file id 1 to its first Regist2 call"
    );
    Ok(())
}

fn validate_frame_selection_storage(file: &object::File<'_>) -> Result<()> {
    validate_main_symbol_relocations(file, "F_Tex", &FRAME_SELECTION_STORAGE_RELOCATIONS)?;
    for (owner, selection, upper, lower, add, load, store) in [
        (
            "Puyo_1P_Init",
            "FrameTexTbl_1P_P",
            0x44,
            0x4c,
            0x60,
            0x64,
            0x6c,
        ),
        (
            "Puyo_1P_Init",
            "FrameTexTbl_1P_N",
            0x80,
            0x88,
            0x8c,
            0x90,
            0x98,
        ),
        (
            "Puyo_1P_Init",
            "FrameTexTbl_1P_N",
            0xa8,
            0xac,
            0xbc,
            0xc0,
            0xc8,
        ),
        (
            "Puyo_1P_Init",
            "FrameTexTbl_1P_T",
            0xd8,
            0xdc,
            0xe4,
            0xe8,
            0xec,
        ),
        (
            "Puyo_2P_Run",
            "FrameTexTbl_2P",
            0x490,
            0x498,
            0x4a4,
            0x4a8,
            0x4ac,
        ),
        (
            "Puyo_2P_Init",
            "FrameTexTbl_2P",
            0x20,
            0x24,
            0x34,
            0x38,
            0x3c,
        ),
    ] {
        validate_frame_selection_store(file, owner, selection, upper, lower, add, load, store)?;
    }
    for (symbol, relocations) in [
        (
            "FrameTexTbl_1P_P",
            vec![
                (0x0047_6ef4, object::elf::R_MIPS_HI16),
                (0x0047_6efc, object::elf::R_MIPS_LO16),
            ],
        ),
        (
            "FrameTexTbl_1P_N",
            vec![
                (0x0047_6f30, object::elf::R_MIPS_HI16),
                (0x0047_6f38, object::elf::R_MIPS_LO16),
                (0x0047_6f58, object::elf::R_MIPS_HI16),
                (0x0047_6f5c, object::elf::R_MIPS_LO16),
            ],
        ),
        (
            "FrameTexTbl_1P_T",
            vec![
                (0x0045_0534, object::elf::R_MIPS_HI16),
                (0x0045_053c, object::elf::R_MIPS_LO16),
                (0x0045_08bc, object::elf::R_MIPS_HI16),
                (0x0045_08c4, object::elf::R_MIPS_LO16),
                (0x0047_6f88, object::elf::R_MIPS_HI16),
                (0x0047_6f8c, object::elf::R_MIPS_LO16),
            ],
        ),
        (
            "FrameTexTbl_2P",
            vec![
                (0x0046_c860, object::elf::R_MIPS_HI16),
                (0x0046_c868, object::elf::R_MIPS_LO16),
                (0x0046_cd20, object::elf::R_MIPS_HI16),
                (0x0046_cd24, object::elf::R_MIPS_LO16),
            ],
        ),
    ] {
        validate_main_symbol_relocations(file, symbol, &relocations)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_frame_selection_store(
    file: &object::File<'_>,
    owner_symbol: &str,
    selection_symbol: &str,
    upper_offset: usize,
    lower_offset: usize,
    add_offset: usize,
    load_offset: usize,
    store_offset: usize,
) -> Result<()> {
    let (owner_address, owner_bytes) = read_symbol_bytes(file, owner_symbol)?;
    let (selection_address, _) = read_symbol_bytes(file, selection_symbol)?;
    validate_loaded_address(
        &owner_bytes,
        owner_address,
        upper_offset,
        lower_offset,
        GeneralRegister::V0,
        selection_address,
        "stored frame-selection table",
    )?;
    require_three_general(
        &owner_bytes,
        owner_address,
        add_offset,
        InstructionForm::Addu,
        GeneralRegister::V0,
        GeneralRegister::V0,
        GeneralRegister::V1,
        "stored frame-selection pointer address",
    )?;
    require_memory(
        &owner_bytes,
        owner_address,
        load_offset,
        InstructionForm::Lw,
        GeneralRegister::V0,
        GeneralRegister::V0,
        0,
        "stored frame-selection pointer load",
    )?;
    require_memory(
        &owner_bytes,
        owner_address,
        store_offset,
        InstructionForm::Sw,
        GeneralRegister::V0,
        GeneralRegister::GP,
        -0x7428,
        "stored selected frame load table",
    )?;
    Ok(())
}

fn validate_stored_selection_regist2_after_reset(
    file: &object::File<'_>,
    owner_symbol: &str,
    init_call_offset: usize,
    table_load_offset: usize,
    registrar_call_offset: usize,
) -> Result<()> {
    let (owner_address, owner_bytes) = read_symbol_bytes(file, owner_symbol)?;
    let (init_address, _) = read_symbol_bytes(file, "ActRegList_Init")?;
    let (registrar_address, _) = read_symbol_bytes(file, "ActRegList_Regist2")?;
    validate_jump_target(
        &owner_bytes,
        owner_address,
        init_call_offset,
        init_address,
        "stored frame Regist2 slot reset",
    )?;
    require_memory(
        &owner_bytes,
        owner_address,
        table_load_offset,
        InstructionForm::Lw,
        GeneralRegister::A1,
        GeneralRegister::GP,
        -0x7428,
        "stored selected frame load table",
    )?;
    validate_jump_target(
        &owner_bytes,
        owner_address,
        registrar_call_offset,
        registrar_address,
        "stored selected frame Regist2 registration",
    )?;
    validate_regist2_image_file_id(
        &owner_bytes,
        owner_address,
        registrar_call_offset,
        owner_symbol,
    )?;
    validate_control_continuity_between_registration_calls(
        &owner_bytes,
        owner_address,
        init_call_offset,
        registrar_call_offset,
        owner_symbol,
    )
}

fn validate_frame_table_relocation_population(
    file: &object::File<'_>,
    frame_tables: &BTreeMap<String, u64>,
) -> Result<()> {
    let targets = frame_tables.values().copied().collect::<BTreeSet<_>>();
    let mut expected = Vec::new();
    for (selection_symbol, _) in FRAME_SELECTION_TABLES {
        let (address, bytes) = read_symbol_bytes(file, selection_symbol)?;
        for (index, pointer) in bytes.chunks_exact(4).enumerate() {
            expected.push((
                address + u64::try_from(index * 4)?,
                object::elf::R_MIPS_32,
                u64::from(u32::from_le_bytes(pointer.try_into()?)),
            ));
        }
    }
    for (address, r_type, target) in [
        (
            0x0043_ab08,
            object::elf::R_MIPS_HI16,
            frame_tables["FrameTexTbl_05"],
        ),
        (
            0x0043_ab14,
            object::elf::R_MIPS_LO16,
            frame_tables["FrameTexTbl_05"],
        ),
        (
            0x0043_ad14,
            object::elf::R_MIPS_HI16,
            frame_tables["FrameTexTbl_05"],
        ),
        (
            0x0043_ad24,
            object::elf::R_MIPS_LO16,
            frame_tables["FrameTexTbl_05"],
        ),
        (
            0x0045_c968,
            object::elf::R_MIPS_HI16,
            frame_tables["FrameTexTbl_00"],
        ),
        (
            0x0045_c974,
            object::elf::R_MIPS_LO16,
            frame_tables["FrameTexTbl_00"],
        ),
        (
            0x0045_ce84,
            object::elf::R_MIPS_HI16,
            frame_tables["FrameTexTbl_00"],
        ),
        (
            0x0045_ce94,
            object::elf::R_MIPS_LO16,
            frame_tables["FrameTexTbl_00"],
        ),
    ] {
        expected.push((address, r_type, target));
    }
    expected.sort_unstable();
    let actual = main_relocations_to_addresses(file, &targets)?;
    ensure!(
        actual == expected,
        ".relmain frame load-table relocation population changed"
    );
    Ok(())
}

#[cfg(test)]
#[path = "registration_tests.rs"]
mod registration_tests;
