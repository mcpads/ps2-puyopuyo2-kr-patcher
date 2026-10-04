use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};
use r5900::{GeneralRegister, InstructionForm, Operands};

use super::NazoStartMenuCallerEvidence;
use super::catalog::{DirectCallSite, FixedTableEntryReferenceSpec, START_MENU_CALLERS};
use crate::cpx::read_symbol_bytes;

use super::super::super::instruction::{
    decode_instruction, instruction_evidence, instruction_word, validate_addiu,
    validate_jump_target,
};
use super::super::super::model::{MessageEntry, TypedInstructionEvidence};

pub(super) fn validate_start_menu_callers(
    file: &object::File<'_>,
) -> Result<Vec<NazoStartMenuCallerEvidence>> {
    let (start_menu_address, _) = read_symbol_bytes(file, "Make_Nazo_StartMenu")?;
    let (caller_address, caller_bytes) = read_symbol_bytes(file, "Nazo_Menu_Run")?;
    let actual = find_direct_call_sites(file, start_menu_address)?;
    let expected: Vec<_> = START_MENU_CALLERS
        .iter()
        .map(|caller| DirectCallSite {
            caller_address,
            call_offset: caller.call_offset,
        })
        .collect();
    ensure!(
        actual == expected,
        "Make_Nazo_StartMenu direct-call population differs from its three declared modes"
    );

    let mut evidence = Vec::with_capacity(START_MENU_CALLERS.len());
    for caller in START_MENU_CALLERS {
        let mut instructions = vec![validate_jump_target(
            &caller_bytes,
            caller_address,
            caller.call_offset,
            start_menu_address,
            "Nazo start-menu constructor",
        )?];
        if caller.zero_move {
            instructions.push(validate_zero_move(
                &caller_bytes,
                caller_address,
                caller.argument_offset,
                GeneralRegister::A0,
                "Nazo start-menu mode zero",
            )?);
        } else {
            instructions.push(validate_addiu(
                &caller_bytes,
                caller_address,
                caller.argument_offset,
                GeneralRegister::A0,
                GeneralRegister::ZERO,
                i16::from(caller.mode),
                "Nazo start-menu mode",
            )?);
        }
        instructions.sort_by(|left, right| left.address.cmp(&right.address));
        evidence.push(NazoStartMenuCallerEvidence {
            caller_symbol: "Nazo_Menu_Run",
            address: format!("{:#010x}", caller_address + caller.call_offset as u64),
            mode: caller.mode,
            instructions,
        });
    }
    Ok(evidence)
}

fn find_direct_call_sites(file: &object::File<'_>, target: u64) -> Result<Vec<DirectCallSite>> {
    let mut calls = Vec::new();
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
        for offset in (0..source.len().saturating_sub(3)).step_by(4) {
            if instruction_word(source, offset).is_none_or(|word| word >> 26 != 0x03) {
                continue;
            }
            let instruction = decode_instruction(source, symbol.address(), offset)?;
            if matches!(instruction.operands(), Operands::Jump { target: actual } if u64::from(*actual) == target)
            {
                calls.push(DirectCallSite {
                    caller_address: symbol.address(),
                    call_offset: offset,
                });
            }
        }
    }
    calls.sort_by_key(|call| (call.caller_address, call.call_offset));
    Ok(calls)
}

fn validate_zero_move(
    source: &[u8],
    start_address: u64,
    offset: usize,
    destination: GeneralRegister,
    role: &str,
) -> Result<TypedInstructionEvidence> {
    let instruction = decode_instruction(source, start_address, offset)?;
    ensure!(
        instruction.form() == InstructionForm::Daddu,
        "{role} is not daddu"
    );
    ensure!(
        matches!(
            instruction.operands(),
            Operands::ThreeGeneral { destination: actual_destination, left, right }
                if *actual_destination == destination
                    && *left == GeneralRegister::ZERO
                    && *right == GeneralRegister::ZERO
        ),
        "{role} operands changed"
    );
    Ok(instruction_evidence(
        source,
        start_address,
        offset,
        &instruction,
    ))
}

pub(super) fn validate_nazo_renderer(
    source: &[u8],
    start_address: u64,
) -> Result<Vec<TypedInstructionEvidence>> {
    Ok(vec![
        validate_addiu(
            source,
            start_address,
            0x88,
            GeneralRegister::V1,
            GeneralRegister::ZERO,
            10,
            "Nazo line-break byte",
        )?,
        validate_addiu(
            source,
            start_address,
            0x94,
            GeneralRegister::S1,
            GeneralRegister::S1,
            1,
            "Nazo line-break input advance",
        )?,
        validate_zero_move(
            source,
            start_address,
            0x98,
            GeneralRegister::S4,
            "Nazo line-break x reset",
        )?,
        validate_addiu(
            source,
            start_address,
            0xa0,
            GeneralRegister::S3,
            GeneralRegister::S3,
            32,
            "Nazo line height",
        )?,
    ])
}

pub(super) fn validate_fixed_table_entry(
    source: &[u8],
    start_address: u64,
    spec: FixedTableEntryReferenceSpec,
    table_address: u64,
) -> Result<Vec<TypedInstructionEvidence>> {
    let upper = decode_instruction(source, start_address, spec.upper_offset)?;
    let upper_immediate = match upper.operands() {
        Operands::LoadUpper {
            destination,
            immediate,
        } if *destination == spec.table_base_register => *immediate,
        _ => anyhow::bail!("fixed Nazo table-entry upper address instruction changed"),
    };
    ensure!(
        upper.form() == InstructionForm::Lui,
        "fixed Nazo table-entry address does not begin with lui"
    );
    let load = decode_instruction(source, start_address, spec.load_offset)?;
    let load_offset = match load.operands() {
        Operands::MemoryGeneral {
            register,
            base,
            offset,
        } if *register == spec.message_register && *base == spec.table_base_register => *offset,
        _ => anyhow::bail!("fixed Nazo table-entry load operands changed"),
    };
    ensure!(
        load.form() == InstructionForm::Lw,
        "fixed Nazo table entry is not loaded with lw"
    );
    let resolved = (u32::from(upper_immediate) << 16).wrapping_add_signed(i32::from(load_offset));
    let expected = table_address
        .checked_add(u64::try_from(spec.table_index * 4)?)
        .context("fixed Nazo table-entry address overflow")?;
    ensure!(
        u64::from(resolved) == expected,
        "fixed Nazo table-entry load no longer resolves to its declared index"
    );
    Ok(vec![
        instruction_evidence(source, start_address, spec.upper_offset, &upper),
        instruction_evidence(source, start_address, spec.load_offset, &load),
    ])
}

pub(super) fn validate_message_token_scope(messages: &[MessageEntry]) -> Result<usize> {
    let mut line_breaks = 0_usize;
    for message in messages {
        for token in &message.tokens {
            ensure!(
                matches!(token.kind, "shift-jis-text" | "ascii-text" | "line-break"),
                "Nazo message {} contains an unsupported token kind: {}",
                message.message_id,
                token.kind
            );
            if token.kind == "line-break" {
                line_breaks += 1;
            }
        }
    }
    Ok(line_breaks)
}
