use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};
use r5900::{GeneralRegister, InstructionForm, Operands};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::executable::validate_r5900_instruction;
use crate::indexed_texture::{TextureRegistrationPair, parse_texture_registration_pairs};
use crate::texture_registration::validate_registration_buffer_layout;
use crate::typed_evidence::{decode_instruction, instruction_evidence};

pub(super) fn validate_registration_path(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<NazoFontRegistrationEvidence> {
    let mut registration_sets = Vec::new();
    let mut set_addresses = BTreeMap::new();
    for symbol in REGISTRATION_SET_SYMBOLS {
        let (address, bytes) = read_symbol_bytes(file, symbol)?;
        let pairs = parse_texture_registration_pairs(symbol, &bytes)?;
        ensure!(
            pairs.get(FONT_REGISTRATION_SLOT)
                == Some(&TextureRegistrationPair {
                    image_load_index: FONT_IMAGE_LOAD_INDEX,
                    palette_slot: FONT_PALETTE_SLOT,
                }),
            "{symbol} no longer registers the shared Nazo font in RegBuff slot 8"
        );
        ensure!(
            set_addresses.insert(address, symbol).is_none(),
            "duplicate Nazo texture-registration address"
        );
        registration_sets.push(NazoTextureRegistrationSet {
            symbol: symbol.to_string(),
            pairs: pairs
                .into_iter()
                .map(|pair| NazoTextureRegistrationPair {
                    image_load_index: pair.image_load_index,
                    palette_slot: pair.palette_slot,
                })
                .collect(),
        });
    }

    let call_sites = find_registration_calls(file, &set_addresses)?;
    let actual_counts = call_sites.iter().fold(BTreeMap::new(), |mut counts, site| {
        *counts
            .entry(site.registration_set_symbol.as_str())
            .or_insert(0_usize) += 1;
        counts
    });
    let expected_counts = BTreeMap::from([
        ("Nazo_TexReg_Menu", 3_usize),
        ("Nazo_TexReg_Nazo", 4),
        ("Nazo_TexReg_Game", 1),
        ("Nazo_TexReg_Edit_0", 3),
        ("Nazo_TexReg_Edit_1", 1),
        ("Nazo_TexReg_Edit_2", 1),
        ("Nazo_TexReg_Edit_4", 3),
        ("Nazo_TexReg_Present", 1),
    ]);
    ensure!(
        actual_counts == expected_counts && call_sites.len() == 17,
        "Nazo texture-registration call population changed"
    );

    let (_, buffer_layout_instructions) = validate_registration_buffer_layout(file, executable)?;

    Ok(NazoFontRegistrationEvidence {
        registrar_symbol: REGISTRAR_SYMBOL.to_string(),
        registration_buffer_symbol: REGISTRATION_BUFFER_SYMBOL.to_string(),
        record_byte_length: REGISTRATION_RECORD_BYTES,
        font_registration_slot: FONT_REGISTRATION_SLOT,
        font_image_load_index: FONT_IMAGE_LOAD_INDEX,
        font_palette_slot: FONT_PALETTE_SLOT,
        registration_sets,
        call_sites,
        buffer_layout_instructions,
    })
}

fn find_registration_calls(
    file: &object::File<'_>,
    set_addresses: &BTreeMap<u64, &str>,
) -> Result<Vec<NazoTextureRegistrationCall>> {
    let (load_table_address, _) = read_symbol_bytes(file, LOAD_TABLE_SYMBOL)?;
    let (registrar_address, _) = read_symbol_bytes(file, REGISTRAR_SYMBOL)?;
    let registrar_word = 0x0c00_0000_u32 | ((u32::try_from(registrar_address)? >> 2) & 0x03ff_ffff);
    let mut calls = Vec::new();
    for symbol in file.symbols() {
        if !symbol.is_definition() || symbol.kind() != SymbolKind::Text || symbol.size() == 0 {
            continue;
        }
        let owner_symbol = symbol
            .name()
            .context("ELF function symbol name is invalid")?;
        let section_index = symbol
            .section_index()
            .with_context(|| format!("ELF function has no section: {owner_symbol}"))?;
        let section = file.section_by_index(section_index)?;
        let relative = usize::try_from(symbol.address() - section.address())?;
        let byte_length = usize::try_from(symbol.size())?;
        let bytes = section
            .data()?
            .get(relative..relative + byte_length)
            .with_context(|| format!("ELF function exceeds section: {owner_symbol}"))?;
        for call_offset in (16..bytes.len().saturating_sub(4)).step_by(4) {
            let word = u32::from_le_bytes(bytes[call_offset..call_offset + 4].try_into()?);
            if word != registrar_word {
                continue;
            }
            let call = validate_r5900_instruction(
                &bytes[call_offset..call_offset + 4],
                symbol.address() + u64::try_from(call_offset)?,
            )?;
            if !matches!(
                call.operands(),
                Operands::Jump { target } if u64::from(*target) == registrar_address
            ) || call.form() != InstructionForm::Jal
            {
                continue;
            }
            let upper_load = decode_instruction(bytes, symbol.address(), call_offset - 16)?;
            let upper_set = decode_instruction(bytes, symbol.address(), call_offset - 12)?;
            let file_id = decode_instruction(bytes, symbol.address(), call_offset - 8)?;
            let lower_load = decode_instruction(bytes, symbol.address(), call_offset - 4)?;
            let lower_set = decode_instruction(bytes, symbol.address(), call_offset + 4)?;
            let Some(actual_load_table) =
                resolved_lui_addiu(&upper_load, &lower_load, GeneralRegister::A1)
            else {
                continue;
            };
            if u64::from(actual_load_table) != load_table_address {
                continue;
            }
            ensure!(
                matches!(
                    file_id.operands(),
                    Operands::SignedImmediate {
                        destination: GeneralRegister::A0,
                        source: GeneralRegister::ZERO,
                        immediate: 1,
                    }
                ) && file_id.form() == InstructionForm::Addiu,
                "Nazo registration no longer selects file id 1"
            );
            let registration_set = resolved_lui_addiu(&upper_set, &lower_set, GeneralRegister::A2)
                .context("Nazo registration set is not materialized with lui/addiu")?;
            let registration_set_symbol = set_addresses
                .get(&u64::from(registration_set))
                .with_context(|| {
                    format!("unknown Nazo registration set address: {registration_set:#x}")
                })?;
            let offsets = [
                call_offset - 16,
                call_offset - 12,
                call_offset - 8,
                call_offset - 4,
                call_offset,
                call_offset + 4,
            ];
            let instructions = offsets
                .into_iter()
                .map(|offset| {
                    let instruction = decode_instruction(bytes, symbol.address(), offset)?;
                    Ok(instruction_evidence(
                        &bytes[offset..offset + 4],
                        symbol.address() + u64::try_from(offset)?,
                        &instruction,
                    ))
                })
                .collect::<Result<_>>()?;
            calls.push(NazoTextureRegistrationCall {
                owner_symbol: owner_symbol.to_string(),
                address: format!("{:#010x}", symbol.address() + u64::try_from(call_offset)?),
                registration_set_symbol: (*registration_set_symbol).to_string(),
                instructions,
            });
        }
    }
    calls.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(calls)
}

fn resolved_lui_addiu(
    upper: &r5900::Instruction,
    lower: &r5900::Instruction,
    register: GeneralRegister,
) -> Option<u32> {
    let Operands::LoadUpper {
        destination: upper_destination,
        immediate: upper_immediate,
    } = upper.operands()
    else {
        return None;
    };
    let Operands::SignedImmediate {
        destination: lower_destination,
        source,
        immediate: lower_immediate,
    } = lower.operands()
    else {
        return None;
    };
    if upper.form() != InstructionForm::Lui
        || lower.form() != InstructionForm::Addiu
        || *upper_destination != register
        || *lower_destination != register
        || *source != register
    {
        return None;
    }
    Some((u32::from(*upper_immediate) << 16).wrapping_add_signed(i32::from(*lower_immediate)))
}
