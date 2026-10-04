use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::Serialize;

use crate::executable::{disassemble_symbol, read_executable_symbol};
pub use crate::executable_references::{DirectCallReference, StaticReference, StaticReferenceKind};
use crate::executable_references::{find_direct_calls, find_static_references};
use crate::indexed_texture::sha256;
use crate::message_corpus::{
    MessageCorpusInputs, survey_memory_card_messages, survey_memory_card_save_messages,
    survey_my_nazo_error_messages, survey_nazo_message_producers, survey_tutorial_messages,
};

#[derive(Clone, Debug)]
pub struct ExecutableFontConsumerInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableFontConsumerReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub scope: ExecutableFontConsumerScope,
    pub summary: ExecutableFontConsumerSummary,
    pub families: Vec<FontConsumerFamily>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableFontConsumerScope {
    pub included: &'static str,
    pub excluded: &'static str,
    pub population_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableFontConsumerSummary {
    pub font_families: usize,
    pub typed_lookup_calls: usize,
    pub renderer_address_references: usize,
    pub font_table_references: usize,
    pub unexpected_lookup_callers: usize,
    pub possible_indirect_lookup_references: usize,
    pub renderer_producer_count_mismatches: usize,
    pub unexpected_font_table_owners: usize,
    pub unresolved_font_consumer_paths: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontConsumerFamily {
    pub family: &'static str,
    pub encoding_role: &'static str,
    pub renderer: ExecutableSymbolEvidence,
    pub lookup: ExecutableSymbolEvidence,
    pub font_table: ExecutableSymbolEvidence,
    pub lookup_calls: Vec<DirectCallReference>,
    pub lookup_static_references: Vec<StaticReference>,
    pub renderer_static_references: Vec<StaticReference>,
    pub renderer_producer_counts: Vec<RendererProducerReferenceCount>,
    pub font_table_static_references: Vec<StaticReference>,
    pub unexpected_lookup_callers: Vec<DirectCallReference>,
    pub unexpected_font_table_owners: Vec<StaticReference>,
    pub lookup_population_status: &'static str,
    pub renderer_population_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RendererProducerReferenceCount {
    pub producer_symbol: String,
    pub declared_actor_creations: usize,
    pub observed_static_references: usize,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableSymbolEvidence {
    pub symbol: &'static str,
    pub address: String,
    pub file_offset: String,
    pub byte_length: usize,
    pub sha256: String,
    pub typed_instruction_count: Option<usize>,
}

#[derive(Clone, Copy)]
struct FontConsumerSpec {
    family: &'static str,
    encoding_role: &'static str,
    renderer_symbol: &'static str,
    lookup_symbol: &'static str,
    font_table_symbol: &'static str,
}

const FONT_CONSUMERS: [FontConsumerSpec; 3] = [
    FontConsumerSpec {
        family: "main_message",
        encoding_role: "Shift-JIS fullwidth and ASCII halfwidth message glyph selection",
        renderer_symbol: "MsgPrint_Act",
        lookup_symbol: "GetFontParam",
        font_table_symbol: "FontList_Zen",
    },
    FontConsumerSpec {
        family: "tutorial_message",
        encoding_role: "tutorial-specific Shift-JIS fullwidth glyph selection",
        renderer_symbol: "ExpMsgPrint_Act",
        lookup_symbol: "GetFontParam_Exp",
        font_table_symbol: "FontList_Exp",
    },
    FontConsumerSpec {
        family: "nazo_message",
        encoding_role: "Nazo-specific Shift-JIS fullwidth and ASCII halfwidth glyph selection",
        renderer_symbol: "NazoMsgPrint_Act",
        lookup_symbol: "GetFontParam_Nazo",
        font_table_symbol: "FontList_Nazo",
    },
];

pub fn survey_executable_font_consumers(
    inputs: ExecutableFontConsumerInputs<'_>,
) -> Result<ExecutableFontConsumerReport> {
    ensure!(
        sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    let file = object::File::parse(inputs.executable)
        .context("failed to parse executable for font-consumer survey")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips && file.endianness() == Endianness::Little,
        "font-consumer survey requires a little-endian MIPS ELF"
    );

    let symbol_addresses = collect_symbol_addresses(inputs.executable)?;
    let lookup_addresses = FONT_CONSUMERS
        .iter()
        .map(|spec| {
            Ok((
                symbol_addresses[spec.lookup_symbol],
                spec.lookup_symbol.to_string(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let all_addresses = FONT_CONSUMERS
        .iter()
        .flat_map(|spec| {
            [
                (spec.renderer_symbol, symbol_addresses[spec.renderer_symbol]),
                (spec.lookup_symbol, symbol_addresses[spec.lookup_symbol]),
                (
                    spec.font_table_symbol,
                    symbol_addresses[spec.font_table_symbol],
                ),
            ]
        })
        .map(|(symbol, address)| (address, symbol.to_string()))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        all_addresses.len() == FONT_CONSUMERS.len() * 3,
        "font-consumer symbols do not have unique addresses"
    );

    let direct_calls = find_direct_calls(&file, &lookup_addresses)?;
    let static_references = find_static_references(&file, &all_addresses)?;
    let declared_renderer_producers = collect_declared_renderer_producers(&inputs)?;
    let mut families = Vec::with_capacity(FONT_CONSUMERS.len());
    for spec in FONT_CONSUMERS {
        let lookup_calls = direct_calls
            .iter()
            .filter(|call| call.target_symbol == spec.lookup_symbol)
            .cloned()
            .collect::<Vec<_>>();
        let lookup_static_references = references_for(&static_references, spec.lookup_symbol);
        let renderer_static_references = references_for(&static_references, spec.renderer_symbol);
        let font_table_static_references =
            references_for(&static_references, spec.font_table_symbol);
        let renderer_producer_counts = compare_renderer_producer_counts(
            &renderer_static_references,
            &declared_renderer_producers[spec.family],
        );
        let unexpected_lookup_callers = lookup_calls
            .iter()
            .filter(|call| {
                !call
                    .owner_symbols
                    .iter()
                    .any(|owner| owner == spec.renderer_symbol)
            })
            .cloned()
            .collect::<Vec<_>>();
        let unexpected_font_table_owners = font_table_static_references
            .iter()
            .filter(|reference| reference.owner_symbol != spec.lookup_symbol)
            .cloned()
            .collect::<Vec<_>>();
        let lookup_population_status =
            if unexpected_lookup_callers.is_empty() && lookup_static_references.is_empty() {
                "complete for direct jal and static pointer/address-materialization lookup uses"
            } else {
                "open: a lookup use is not the declared renderer's direct jal"
            };
        let renderer_population_status = if renderer_producer_counts
            .iter()
            .all(|count| count.declared_actor_creations == count.observed_static_references)
        {
            "complete: static renderer references match every producer-specific actor-creation census"
        } else {
            "open: static renderer references differ from a producer-specific actor-creation census"
        };
        families.push(FontConsumerFamily {
            family: spec.family,
            encoding_role: spec.encoding_role,
            renderer: symbol_evidence(inputs.executable, spec.renderer_symbol, true)?,
            lookup: symbol_evidence(inputs.executable, spec.lookup_symbol, true)?,
            font_table: symbol_evidence(inputs.executable, spec.font_table_symbol, false)?,
            lookup_calls,
            lookup_static_references,
            renderer_static_references,
            renderer_producer_counts,
            font_table_static_references,
            unexpected_lookup_callers,
            unexpected_font_table_owners,
            lookup_population_status,
            renderer_population_status,
        });
    }

    let typed_lookup_calls = families
        .iter()
        .map(|family| family.lookup_calls.len())
        .sum();
    let renderer_address_references = families
        .iter()
        .map(|family| family.renderer_static_references.len())
        .sum();
    let font_table_references = families
        .iter()
        .map(|family| family.font_table_static_references.len())
        .sum();
    let unexpected_lookup_callers = families
        .iter()
        .map(|family| family.unexpected_lookup_callers.len())
        .sum();
    let possible_indirect_lookup_references = families
        .iter()
        .map(|family| family.lookup_static_references.len())
        .sum();
    let renderer_producer_count_mismatches = families
        .iter()
        .flat_map(|family| &family.renderer_producer_counts)
        .filter(|count| count.declared_actor_creations != count.observed_static_references)
        .count();
    let unexpected_font_table_owners = families
        .iter()
        .map(|family| family.unexpected_font_table_owners.len())
        .sum();
    let unresolved_font_consumer_paths = unexpected_lookup_callers
        + possible_indirect_lookup_references
        + renderer_producer_count_mismatches
        + unexpected_font_table_owners;
    let population_status = if unresolved_font_consumer_paths == 0 {
        "complete for the three declared Shift-JIS font families across direct lookup calls, static lookup/table references, and producer-specific renderer actor creations"
    } else {
        "open: one or more declared font-consumer paths require classification"
    };

    Ok(ExecutableFontConsumerReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        scope: ExecutableFontConsumerScope {
            included: "all direct jal calls and aligned pointer or typed lui/addiu references to the three declared Shift-JIS font lookups, their renderers, and fullwidth repertoire tables in named file-backed ELF symbols; renderer references are compared with every producer-specific actor-creation census",
            excluded: "indirect values computed beyond the static reference forms, message pointer provenance beyond the producer-specific corpora, single-byte aqMessageDraw/OptPrint families, runtime-built text, graphics, video, and runtime reachability",
            population_status,
        },
        summary: ExecutableFontConsumerSummary {
            font_families: families.len(),
            typed_lookup_calls,
            renderer_address_references,
            font_table_references,
            unexpected_lookup_callers,
            possible_indirect_lookup_references,
            renderer_producer_count_mismatches,
            unexpected_font_table_owners,
            unresolved_font_consumer_paths,
        },
        families,
    })
}

fn collect_declared_renderer_producers(
    inputs: &ExecutableFontConsumerInputs<'_>,
) -> Result<BTreeMap<&'static str, BTreeMap<String, usize>>> {
    let load = survey_memory_card_messages(message_inputs(inputs))?;
    let save = survey_memory_card_save_messages(message_inputs(inputs))?;
    let my_nazo = survey_my_nazo_error_messages(message_inputs(inputs))?;
    let tutorial = survey_tutorial_messages(message_inputs(inputs))?;
    let nazo = survey_nazo_message_producers(message_inputs(inputs))?;

    let main = [
        (
            load.consumer.consumer_symbol.to_string(),
            load.population.consumer_message_sites,
        ),
        (
            save.consumer.consumer_symbol.to_string(),
            save.population.consumer_message_sites,
        ),
        (
            my_nazo.consumer.consumer_symbol.to_string(),
            my_nazo.population.consumer_message_sites,
        ),
    ]
    .into_iter()
    .collect();
    let tutorial = BTreeMap::from([(
        tutorial.producer.symbol.to_string(),
        tutorial.producer.renderer_actor_creations,
    )]);
    let nazo = nazo
        .producers
        .into_iter()
        .map(|producer| (producer.symbol.to_string(), producer.actor_creations.len()))
        .collect();
    Ok(BTreeMap::from([
        ("main_message", main),
        ("tutorial_message", tutorial),
        ("nazo_message", nazo),
    ]))
}

fn message_inputs<'a>(inputs: &'a ExecutableFontConsumerInputs<'a>) -> MessageCorpusInputs<'a> {
    MessageCorpusInputs {
        source_bin_sha256: inputs.source_bin_sha256.clone(),
        executable_iso_path: inputs.executable_iso_path.clone(),
        executable_sha256: inputs.executable_sha256.clone(),
        executable: inputs.executable,
    }
}

fn compare_renderer_producer_counts(
    references: &[StaticReference],
    declared: &BTreeMap<String, usize>,
) -> Vec<RendererProducerReferenceCount> {
    let mut observed = BTreeMap::<String, usize>::new();
    for reference in references {
        *observed.entry(reference.owner_symbol.clone()).or_default() += 1;
    }
    let producers = declared
        .keys()
        .chain(observed.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    producers
        .into_iter()
        .map(|producer_symbol| {
            let declared_actor_creations = declared.get(&producer_symbol).copied().unwrap_or(0);
            let observed_static_references = observed.get(&producer_symbol).copied().unwrap_or(0);
            let status = if declared_actor_creations == observed_static_references {
                "matched"
            } else {
                "mismatch"
            };
            RendererProducerReferenceCount {
                producer_symbol,
                declared_actor_creations,
                observed_static_references,
                status,
            }
        })
        .collect()
}

fn collect_symbol_addresses(executable: &[u8]) -> Result<BTreeMap<&'static str, u32>> {
    FONT_CONSUMERS
        .iter()
        .flat_map(|spec| {
            [
                spec.renderer_symbol,
                spec.lookup_symbol,
                spec.font_table_symbol,
            ]
        })
        .map(|symbol| {
            let address = read_executable_symbol(executable, symbol)?.address;
            Ok((symbol, u32::try_from(address)?))
        })
        .collect()
}

fn references_for(
    references: &BTreeMap<String, Vec<StaticReference>>,
    symbol: &str,
) -> Vec<StaticReference> {
    references.get(symbol).cloned().unwrap_or_default()
}

fn symbol_evidence(
    executable: &[u8],
    symbol: &'static str,
    is_code: bool,
) -> Result<ExecutableSymbolEvidence> {
    let bytes = read_executable_symbol(executable, symbol)?;
    let typed_instruction_count = is_code
        .then(|| {
            disassemble_symbol(executable, symbol).map(|disassembly| disassembly.instructions.len())
        })
        .transpose()?;
    Ok(ExecutableSymbolEvidence {
        symbol,
        address: format!("{:#010x}", bytes.address),
        file_offset: format!("{:#010x}", bytes.file_offset),
        byte_length: bytes.bytes.len(),
        sha256: sha256(&bytes.bytes),
        typed_instruction_count,
    })
}

#[cfg(test)]
#[path = "executable_font_consumers/executable_font_consumer_tests.rs"]
mod executable_font_consumer_tests;
