use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::Serialize;

use super::{NazoMessageProducerReport, RENDERER_SYMBOL, survey_nazo_message_producers};
use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::sha256;

use super::super::ACTOR_SETTER_SYMBOL;
use super::super::catalog::MessageSpec;
use super::super::instruction::{
    validate_actor_creation, validate_loaded_address, validate_message_store,
    validate_pointer_table, validate_table_lookup,
};
use super::super::model::{
    ControlCodeUsage, MessageCorpusInputs, MessageEntry, MessagePopulation,
    MessageTranslationScope, TypedInstructionEvidence,
};
use super::super::parsing::read_message_entry;
use super::super::survey::register_message_reference;

mod catalog;
mod validation;

use catalog::*;
use validation::*;

#[derive(Clone, Debug, Serialize)]
pub struct NazoMessageCorpusReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub scope: NazoMessageCorpusScope,
    pub population: MessagePopulation,
    pub producer_evidence: NazoMessageProducerReport,
    pub start_menu_callers: Vec<NazoStartMenuCallerEvidence>,
    pub renderer_invariants: Vec<TypedInstructionEvidence>,
    pub message_tables: Vec<NazoMessageTableEvidence>,
    pub references: Vec<NazoMessageReferenceEvidence>,
    pub control_codes: Vec<ControlCodeUsage>,
    pub messages: Vec<MessageEntry>,
    pub translation_scope: MessageTranslationScope,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoMessageCorpusScope {
    pub included: &'static str,
    pub excluded: &'static str,
    pub font_status: &'static str,
    pub runtime_status: &'static str,
}

pub(super) fn declared_message_symbols() -> Vec<&'static str> {
    catalog::ALL_MESSAGE_TABLES
        .iter()
        .flat_map(|(_, messages)| messages.iter().map(|message| message.symbol))
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoStartMenuCallerEvidence {
    pub caller_symbol: &'static str,
    pub address: String,
    pub mode: u8,
    pub instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoMessageTableEvidence {
    pub symbol: &'static str,
    pub address: String,
    pub byte_length: usize,
    pub sha256: String,
    pub pointer_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoMessageReferenceEvidence {
    pub reference_id: &'static str,
    pub producer_symbol: &'static str,
    pub actor_creation_address: String,
    pub kind: &'static str,
    pub table_symbol: &'static str,
    pub table_indices: Vec<usize>,
    pub message_ids: Vec<String>,
    pub instructions: Vec<TypedInstructionEvidence>,
}

pub fn survey_nazo_messages(inputs: MessageCorpusInputs<'_>) -> Result<NazoMessageCorpusReport> {
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

    let producer_evidence = survey_nazo_message_producers(MessageCorpusInputs {
        source_bin_sha256: inputs.source_bin_sha256.clone(),
        executable_iso_path: inputs.executable_iso_path.clone(),
        executable_sha256: inputs.executable_sha256.clone(),
        executable: inputs.executable,
    })?;
    let (renderer_address, renderer_bytes) = read_symbol_bytes(&file, RENDERER_SYMBOL)?;
    let (setter_address, _) = read_symbol_bytes(&file, ACTOR_SETTER_SYMBOL)?;
    let start_menu_callers = validate_start_menu_callers(&file)?;
    let renderer_invariants = validate_nazo_renderer(&renderer_bytes, renderer_address)?;

    let mut message_tables = Vec::with_capacity(ALL_MESSAGE_TABLES.len());
    for (table_symbol, messages) in ALL_MESSAGE_TABLES {
        let reference = INDEXED_REFERENCES
            .iter()
            .find(|reference| reference.table.table_symbol == table_symbol)
            .with_context(|| format!("no indexed validation spec for {table_symbol}"))?;
        ensure!(
            reference.table.messages.len() == messages.len(),
            "table message population differs across Nazo catalog declarations"
        );
        let (table_address, table_bytes) = read_symbol_bytes(&file, table_symbol)?;
        validate_pointer_table(&file, reference.table, &table_bytes)?;
        message_tables.push(NazoMessageTableEvidence {
            symbol: table_symbol,
            address: format!("{table_address:#010x}"),
            byte_length: table_bytes.len(),
            sha256: sha256(&table_bytes),
            pointer_count: messages.len(),
        });
    }

    let mut references = Vec::with_capacity(INDEXED_REFERENCES.len() + FIXED_REFERENCES.len());
    let mut messages_by_symbol = BTreeMap::<&str, (MessageSpec, Vec<String>)>::new();
    for reference in INDEXED_REFERENCES {
        let (producer_address, producer_bytes) =
            read_symbol_bytes(&file, reference.producer_symbol)?;
        let (table_address, _) = read_symbol_bytes(&file, reference.table.table_symbol)?;
        let mut instructions = validate_actor_creation(
            &producer_bytes,
            producer_address,
            reference.table.actor,
            renderer_address,
            RENDERER_SYMBOL,
            setter_address,
        )?;
        instructions.extend(
            validate_loaded_address(
                &producer_bytes,
                producer_address,
                reference.table.upper_offset,
                reference.table.lower_offset,
                reference.table.table_register,
                table_address,
                reference.table.table_symbol,
            )
            .with_context(|| reference.table.reference_id)?,
        );
        instructions.extend(
            validate_table_lookup(&producer_bytes, producer_address, reference.table)
                .with_context(|| reference.table.reference_id)?,
        );
        instructions.push(
            validate_message_store(
                &producer_bytes,
                producer_address,
                reference.table.store_offset,
                reference.table.message_register,
                reference.table.store_base,
            )
            .with_context(|| reference.table.reference_id)?,
        );
        instructions.sort_by(|left, right| left.address.cmp(&right.address));

        let mut message_ids = Vec::with_capacity(reference.table_indices.len());
        let mut unique_indices = BTreeSet::new();
        for table_index in reference.table_indices {
            ensure!(
                unique_indices.insert(*table_index),
                "Nazo table reference repeats one physical table index"
            );
            let message = *reference
                .table
                .messages
                .get(*table_index)
                .with_context(|| {
                    format!(
                        "{} index {} exceeds {}",
                        reference.table.reference_id, table_index, reference.table.table_symbol
                    )
                })?;
            register_message_reference(
                &mut messages_by_symbol,
                message,
                reference.table.reference_id,
            )?;
            message_ids.push(message.id.to_string());
        }
        references.push(NazoMessageReferenceEvidence {
            reference_id: reference.table.reference_id,
            producer_symbol: reference.producer_symbol,
            actor_creation_address: format!(
                "{:#010x}",
                producer_address + reference.table.actor.upper_offset as u64
            ),
            kind: "indexed-pointer-table",
            table_symbol: reference.table.table_symbol,
            table_indices: reference.table_indices.to_vec(),
            message_ids,
            instructions,
        });
    }

    for reference in FIXED_REFERENCES {
        let (producer_address, producer_bytes) =
            read_symbol_bytes(&file, reference.producer_symbol)?;
        let (table_address, _) = read_symbol_bytes(&file, reference.table_symbol)?;
        let mut instructions = validate_actor_creation(
            &producer_bytes,
            producer_address,
            reference.actor,
            renderer_address,
            RENDERER_SYMBOL,
            setter_address,
        )?;
        instructions.extend(validate_fixed_table_entry(
            &producer_bytes,
            producer_address,
            reference,
            table_address,
        )?);
        instructions.push(validate_message_store(
            &producer_bytes,
            producer_address,
            reference.store_offset,
            reference.message_register,
            reference.store_base,
        )?);
        instructions.sort_by(|left, right| left.address.cmp(&right.address));
        let message = *reference
            .messages
            .get(reference.table_index)
            .context("fixed Nazo table entry exceeds its table")?;
        register_message_reference(&mut messages_by_symbol, message, reference.reference_id)?;
        references.push(NazoMessageReferenceEvidence {
            reference_id: reference.reference_id,
            producer_symbol: reference.producer_symbol,
            actor_creation_address: format!(
                "{:#010x}",
                producer_address + reference.actor.upper_offset as u64
            ),
            kind: "fixed-pointer-table-entry",
            table_symbol: reference.table_symbol,
            table_indices: vec![reference.table_index],
            message_ids: vec![message.id.to_string()],
            instructions,
        });
    }
    references.sort_by(|left, right| {
        left.actor_creation_address
            .cmp(&right.actor_creation_address)
    });

    let declared_message_count = ALL_MESSAGE_TABLES
        .iter()
        .map(|(_, messages)| messages.len())
        .sum::<usize>();
    ensure!(
        messages_by_symbol.len() == declared_message_count,
        "Nazo consumer references do not cover every declared physical message"
    );
    let mut messages = Vec::with_capacity(messages_by_symbol.len());
    for (_, (spec, mut reference_ids)) in messages_by_symbol {
        reference_ids.sort();
        messages.push(read_message_entry(&file, spec, reference_ids)?);
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
    let protected_english_runs = messages
        .iter()
        .map(|message| message.protected_english_runs.len())
        .sum();
    let line_breaks = validate_message_token_scope(&messages)?;
    let physical_reference_edges = references
        .iter()
        .map(|reference| reference.table_indices.len())
        .sum::<usize>();
    ensure!(
        physical_reference_edges >= messages.len(),
        "Nazo physical reference count is smaller than its unique message count"
    );

    Ok(NazoMessageCorpusReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        scope: NazoMessageCorpusScope {
            included: "all nine direct NazoMsgPrint_Act actor creations, their child +0x108 message stores, four exact pointer tables, and all 36 table-owned strings",
            excluded: "every other renderer, Nazo font lookup and glyph supply, translated assets, and image rebuilding",
            font_status: "NazoMsgPrint_Act font lookup and Korean glyph capacity remain open",
            runtime_status: "static typed consumer proof only; no cold-boot Nazo screen proof",
        },
        population: MessagePopulation {
            status: "complete for the declared direct NazoMsgPrint_Act producer and four-table consumer population",
            consumer_message_sites: references.len(),
            direct_sites: FIXED_REFERENCES.len(),
            indexed_table_sites: INDEXED_REFERENCES.len(),
            physical_reference_edges,
            unique_messages: messages.len(),
            duplicate_reference_edges: physical_reference_edges - messages.len(),
            unresolved_sites: 0,
            raw_payload_bytes,
            japanese_characters,
            protected_english_runs,
        },
        producer_evidence,
        start_menu_callers,
        renderer_invariants,
        message_tables,
        references,
        control_codes: if line_breaks == 0 {
            Vec::new()
        } else {
            vec![ControlCodeUsage {
                kind: "line-break".to_string(),
                raw_values: vec!["0a".to_string()],
                occurrences: line_breaks,
                byte_width: 1,
                policy: "movable-layout".to_string(),
                confirmed_effect:
                    "advance the input by one byte, reset x, and advance y by 32 pixels".to_string(),
            }]
        },
        messages,
        translation_scope: MessageTranslationScope {
            target: "non-ASCII Shift-JIS runs in Japanese Nazo messages",
            preserve: "English and other ASCII runs byte-for-byte, plus line breaks and every value outside the declared Japanese strings",
            report_role: "read-only corpus evidence; not an approved translation asset",
        },
    })
}

#[cfg(test)]
#[path = "corpus_tests.rs"]
mod corpus_tests;
