use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::Serialize;

use super::catalog::{ActorCreationSpec, actor};
use super::instruction::validate_actor_creation;
use super::survey::find_renderer_actor_creation_sites;
use super::{ACTOR_SETTER_SYMBOL, MessageCorpusInputs, TypedInstructionEvidence};
use crate::cpx::read_symbol_bytes;
use crate::executable::disassemble_symbol;
use crate::indexed_texture::sha256;

const RENDERER_SYMBOL: &str = "NazoMsgPrint_Act";

mod corpus;

pub use corpus::{NazoMessageCorpusReport, survey_nazo_messages};

pub(super) fn declared_message_symbols() -> Vec<&'static str> {
    corpus::declared_message_symbols()
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoMessageProducerReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub scope: NazoMessageProducerScope,
    pub population: NazoMessageProducerPopulation,
    pub renderer: NazoRendererEvidence,
    pub producers: Vec<NazoMessageProducerEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoMessageProducerScope {
    pub included: &'static str,
    pub excluded: &'static str,
    pub consumer_status: &'static str,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoMessageProducerPopulation {
    pub status: &'static str,
    pub producer_functions: usize,
    pub renderer_actor_creations: usize,
    pub unresolved_sites: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoRendererEvidence {
    pub isa_profile: &'static str,
    pub symbol: &'static str,
    pub address: String,
    pub byte_length: usize,
    pub sha256: String,
    pub instruction_count: usize,
    pub actor_setter_symbol: &'static str,
    pub actor_setter_address: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoMessageProducerEvidence {
    pub symbol: &'static str,
    pub address: String,
    pub byte_length: usize,
    pub sha256: String,
    pub instruction_count: usize,
    pub actor_creations: Vec<NazoActorCreationEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NazoActorCreationEvidence {
    pub address: String,
    pub upper_offset: usize,
    pub lower_offset: usize,
    pub call_offset: usize,
    pub instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Copy)]
struct ExpectedActorCreation {
    producer_symbol: &'static str,
    producer_address: u64,
    actor: ActorCreationSpec,
}

const EXPECTED_ACTOR_CREATIONS: [ExpectedActorCreation; 9] = [
    expected("Make_Nazo_StartMenu", 0x0045_f120, actor(0x34, 0x40, 0x3c)),
    expected(
        "Make_Nazo_StartMenu",
        0x0045_f120,
        actor(0x238, 0x23c, 0x240),
    ),
    expected(
        "Make_Nazo_StartMenu",
        0x0045_f120,
        actor(0x3bc, 0x3c8, 0x3c4),
    ),
    expected("Make_YesNo_Window", 0x0045_f8c0, actor(0x4a8, 0x4b4, 0x4b0)),
    expected("Nazo_Edit_DataDisp", 0x0046_4930, actor(0x58, 0x60, 0x6c)),
    expected("Make_Nazo_InfoAct", 0x0046_4d50, actor(0x90, 0x9c, 0x98)),
    expected("Nazo_Menu_Run", 0x0046_6640, actor(0x1a70, 0x1a7c, 0x1a78)),
    expected("Nazo_Menu_Run", 0x0046_6640, actor(0x1e2c, 0x1e38, 0x1e34)),
    expected("Nazo_Menu_Run", 0x0046_6640, actor(0x2188, 0x2194, 0x2190)),
];

const PRODUCER_SYMBOLS: [&str; 5] = [
    "Make_Nazo_StartMenu",
    "Make_YesNo_Window",
    "Nazo_Edit_DataDisp",
    "Make_Nazo_InfoAct",
    "Nazo_Menu_Run",
];

const fn expected(
    producer_symbol: &'static str,
    producer_address: u64,
    actor: ActorCreationSpec,
) -> ExpectedActorCreation {
    ExpectedActorCreation {
        producer_symbol,
        producer_address,
        actor,
    }
}

pub fn survey_nazo_message_producers(
    inputs: MessageCorpusInputs<'_>,
) -> Result<NazoMessageProducerReport> {
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

    let renderer_disassembly = disassemble_symbol(inputs.executable, RENDERER_SYMBOL)?;
    let (renderer_address, renderer_bytes) = read_symbol_bytes(&file, RENDERER_SYMBOL)?;
    let (setter_address, _) = read_symbol_bytes(&file, ACTOR_SETTER_SYMBOL)?;
    let actual_sites = find_renderer_actor_creation_sites(&file, renderer_address, setter_address)?;
    ensure!(
        actual_sites.len() == EXPECTED_ACTOR_CREATIONS.len(),
        "NazoMsgPrint_Act actor-creation population changed: expected {}, found {}",
        EXPECTED_ACTOR_CREATIONS.len(),
        actual_sites.len()
    );
    for (actual, expected) in actual_sites.iter().zip(EXPECTED_ACTOR_CREATIONS) {
        ensure!(
            actual.producer_symbol == expected.producer_symbol
                && actual.producer_address == expected.producer_address
                && actual.actor == expected.actor,
            "NazoMsgPrint_Act actor-creation site differs from the fixed producer population"
        );
    }

    let mut producers = Vec::with_capacity(PRODUCER_SYMBOLS.len());
    for producer_symbol in PRODUCER_SYMBOLS {
        let disassembly = disassemble_symbol(inputs.executable, producer_symbol)?;
        let (producer_address, producer_bytes) = read_symbol_bytes(&file, producer_symbol)?;
        let mut actor_creations = Vec::new();
        for site in actual_sites
            .iter()
            .filter(|site| site.producer_symbol == producer_symbol)
        {
            let instructions = validate_actor_creation(
                &producer_bytes,
                producer_address,
                site.actor,
                renderer_address,
                RENDERER_SYMBOL,
                setter_address,
            )?;
            actor_creations.push(NazoActorCreationEvidence {
                address: format!(
                    "{:#010x}",
                    producer_address + site.actor.upper_offset as u64
                ),
                upper_offset: site.actor.upper_offset,
                lower_offset: site.actor.lower_offset,
                call_offset: site.actor.call_offset,
                instructions,
            });
        }
        ensure!(
            !actor_creations.is_empty(),
            "declared Nazo message producer has no actor creation: {producer_symbol}"
        );
        producers.push(NazoMessageProducerEvidence {
            symbol: producer_symbol,
            address: format!("{producer_address:#010x}"),
            byte_length: producer_bytes.len(),
            sha256: sha256(&producer_bytes),
            instruction_count: disassembly.instructions.len(),
            actor_creations,
        });
    }

    Ok(NazoMessageProducerReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        scope: NazoMessageProducerScope {
            included: "all direct NazoMsgPrint_Act address materializations passed to Act_SetBottom in defined ELF functions",
            excluded: "message pointer stores, pointer-table contents, font lookup tables, and every other renderer",
            consumer_status: "producer population only; message consumer census remains open",
            runtime_status: "static actor-creation proof only; no cold-boot screen proof",
        },
        population: NazoMessageProducerPopulation {
            status: "complete for the declared direct NazoMsgPrint_Act actor-creation pattern in the supported executable",
            producer_functions: producers.len(),
            renderer_actor_creations: actual_sites.len(),
            unresolved_sites: 0,
        },
        renderer: NazoRendererEvidence {
            isa_profile: r5900::PROFILE_ID,
            symbol: RENDERER_SYMBOL,
            address: format!("{renderer_address:#010x}"),
            byte_length: renderer_bytes.len(),
            sha256: sha256(&renderer_bytes),
            instruction_count: renderer_disassembly.instructions.len(),
            actor_setter_symbol: ACTOR_SETTER_SYMBOL,
            actor_setter_address: format!("{setter_address:#010x}"),
        },
        producers,
    })
}

#[cfg(test)]
#[path = "nazo/producer_tests.rs"]
mod producer_tests;
