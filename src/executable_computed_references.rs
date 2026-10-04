use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::{Deserialize, Serialize};

mod control_flow;
mod propagation;
mod review;
mod value_flow;

use propagation::scan_text_symbol;
use review::validate_code_data_review;

use crate::executable_string_census::{
    ExecutableStringCensusInputs, ExecutableStringReferenceKind, ExecutableStringReviewCatalog,
    census_executable_strings,
};
use crate::typed_evidence::TypedInstructionEvidence;

pub struct ExecutableComputedReferenceInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExecutableCodeDataReviewCatalog {
    pub schema: u32,
    pub executable_sha256: String,
    pub review_basis: String,
    pub regions: Vec<ExecutableCodeDataReviewRegion>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExecutableCodeDataReviewRegion {
    pub region_id: String,
    pub symbol_name: String,
    pub start_address: String,
    pub end_address: String,
    pub source_sha256: String,
    pub classification: ExecutableCodeDataClassification,
    pub note: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutableCodeDataClassification {
    InlineData,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableComputedReferenceReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub gp_value: String,
    pub summary: ExecutableComputedReferenceSummary,
    pub review_basis: String,
    pub reviewed_code_data: Vec<ExecutableCodeDataReviewedRegion>,
    pub control_flow_boundaries: Vec<ExecutableControlFlowBoundary>,
    pub references: Vec<ExecutableComputedReference>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableComputedReferenceSummary {
    pub japanese_script_candidates: usize,
    pub text_symbol_definitions: usize,
    pub unique_text_functions: usize,
    pub unique_typed_instruction_words: usize,
    pub reviewed_inline_data_words: usize,
    pub straight_line_basic_blocks: usize,
    pub known_value_updates: usize,
    pub direct_control_flow_edges: usize,
    pub control_flow_roots: usize,
    pub control_flow_reachable_instruction_words: usize,
    pub control_flow_merge_instruction_words: usize,
    pub control_flow_fixed_point_state_updates: usize,
    pub control_flow_conservative_boundaries: usize,
    pub candidate_reference_edges: usize,
    pub references_requiring_control_flow_propagation: usize,
    pub static_pointer_load_reference_edges: usize,
    pub gp_derived_reference_edges: usize,
    pub reference_edges_with_more_than_two_instructions: usize,
    pub referenced_candidates: usize,
    pub newly_referenced_candidates: usize,
    pub unresolved_instruction_words: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableCodeDataReviewedRegion {
    pub region_id: String,
    pub symbol_name: String,
    pub start_address: String,
    pub end_address: String,
    pub byte_length: usize,
    pub source_sha256: String,
    pub classification: ExecutableCodeDataClassification,
    pub note: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableControlFlowBoundary {
    pub owner_symbol: String,
    pub instruction_address: String,
    pub delay_slot_address: String,
    pub boundary_kind: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableComputedReference {
    pub owner_symbol: String,
    pub candidate_symbol: String,
    pub candidate_address: String,
    pub previously_referenced: bool,
    pub result_instruction: TypedInstructionEvidence,
    pub evidence: Vec<TypedInstructionEvidence>,
    pub pointer_slot_addresses: Vec<String>,
    pub uses_gp_initial_value: bool,
    pub requires_control_flow_propagation: bool,
}

pub fn survey_executable_computed_references(
    inputs: ExecutableComputedReferenceInputs<'_>,
    string_catalog: &ExecutableStringReviewCatalog,
    code_data_catalog: &ExecutableCodeDataReviewCatalog,
) -> Result<ExecutableComputedReferenceReport> {
    ensure!(
        crate::indexed_texture::sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    let file = object::File::parse(inputs.executable)
        .context("failed to parse executable for computed-reference survey")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips && file.endianness() == Endianness::Little,
        "computed-reference survey requires a little-endian MIPS ELF"
    );
    let census = census_executable_strings(
        ExecutableStringCensusInputs {
            source_bin_sha256: inputs.source_bin_sha256.clone(),
            executable_iso_path: inputs.executable_iso_path.clone(),
            executable_sha256: inputs.executable_sha256.clone(),
            executable: inputs.executable,
        },
        string_catalog,
    )?;
    let candidates = census
        .candidates
        .iter()
        .map(|candidate| {
            Ok((
                parse_u32_address(&candidate.address)?,
                candidate.symbol.clone(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    ensure!(
        candidates.len() == census.candidates.len(),
        "Japanese-script candidate addresses are not unique"
    );
    let previously_referenced = census
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.references.iter().any(|reference| {
                matches!(
                    reference.kind,
                    ExecutableStringReferenceKind::TypedAddressMaterialization
                        | ExecutableStringReferenceKind::Pointer
                )
            })
        })
        .map(|candidate| candidate.symbol.clone())
        .collect::<BTreeSet<_>>();
    let reviewed_regions =
        validate_code_data_review(&file, &inputs.executable_sha256, code_data_catalog)?;
    let gp = crate::executable_text::read_elf_gp_identity(&file)?.value;
    let text = crate::executable_text::collect_file_backed_text(&file)?;
    let candidate_addresses = candidates.keys().copied().collect::<BTreeSet<_>>();
    let mut typed_instruction_addresses = BTreeSet::new();
    let mut reviewed_data_addresses = BTreeSet::new();
    let mut straight_line_basic_blocks = 0;
    let mut known_value_updates = 0;
    let mut direct_control_flow_edges = 0;
    let mut control_flow_roots = 0;
    let mut control_flow_reachable_instruction_addresses = BTreeSet::new();
    let mut control_flow_merge_instruction_addresses = BTreeSet::new();
    let mut control_flow_fixed_point_state_updates = 0;
    let mut control_flow_boundaries =
        BTreeMap::<(String, u64, u64, &'static str), ExecutableControlFlowBoundary>::new();
    let mut references = BTreeMap::<(String, u64, String), ExecutableComputedReference>::new();
    for symbol in &text.symbols {
        let scan = scan_text_symbol(&file, symbol, gp, &candidate_addresses, &reviewed_regions)?;
        typed_instruction_addresses.extend(scan.typed_instruction_addresses);
        reviewed_data_addresses.extend(scan.reviewed_data_addresses);
        straight_line_basic_blocks += scan.basic_blocks;
        known_value_updates += scan.known_value_updates;
        direct_control_flow_edges += scan.direct_control_flow_edges;
        control_flow_roots += scan.control_flow_roots;
        control_flow_reachable_instruction_addresses
            .extend(scan.control_flow_reachable_instruction_addresses);
        control_flow_merge_instruction_addresses
            .extend(scan.control_flow_merge_instruction_addresses);
        control_flow_fixed_point_state_updates += scan.control_flow_fixed_point_state_updates;
        for boundary in scan.control_flow_conservative_boundaries {
            for owner_symbol in &symbol.owners {
                control_flow_boundaries
                    .entry((
                        owner_symbol.clone(),
                        boundary.instruction_address,
                        boundary.delay_slot_address,
                        boundary.boundary_kind,
                    ))
                    .or_insert_with(|| ExecutableControlFlowBoundary {
                        owner_symbol: owner_symbol.clone(),
                        instruction_address: format!("{:#010x}", boundary.instruction_address),
                        delay_slot_address: format!("{:#010x}", boundary.delay_slot_address),
                        boundary_kind: boundary.boundary_kind,
                    });
            }
        }
        let straight_line_productions = scan
            .productions
            .iter()
            .map(|production| {
                (
                    production.candidate_address,
                    production.result_instruction.address.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        let mut productions = scan
            .productions
            .into_iter()
            .map(|production| (production, false))
            .collect::<Vec<_>>();
        productions.extend(
            scan.control_flow_productions
                .into_iter()
                .filter_map(|production| {
                    let key = (
                        production.candidate_address,
                        production.result_instruction.address.clone(),
                    );
                    (!straight_line_productions.contains(&key)).then_some((production, true))
                }),
        );
        for (production, requires_control_flow_propagation) in productions {
            let candidate_symbol = candidates
                .get(&production.candidate_address)
                .context("computed reference names an unknown candidate")?;
            for owner_symbol in &symbol.owners {
                let result_address = parse_u64_address(&production.result_instruction.address)?;
                references
                    .entry((
                        owner_symbol.clone(),
                        result_address,
                        candidate_symbol.clone(),
                    ))
                    .and_modify(|reference| {
                        reference.requires_control_flow_propagation &=
                            requires_control_flow_propagation;
                    })
                    .or_insert_with(|| ExecutableComputedReference {
                        owner_symbol: owner_symbol.clone(),
                        candidate_symbol: candidate_symbol.clone(),
                        candidate_address: format!("{:#010x}", production.candidate_address),
                        previously_referenced: previously_referenced.contains(candidate_symbol),
                        result_instruction: production.result_instruction.clone(),
                        evidence: production.evidence.clone(),
                        pointer_slot_addresses: production
                            .pointer_slot_addresses
                            .iter()
                            .map(|address| format!("{address:#010x}"))
                            .collect(),
                        uses_gp_initial_value: production.uses_gp_initial_value,
                        requires_control_flow_propagation,
                    });
            }
        }
    }
    let expected_reviewed_addresses = reviewed_regions
        .iter()
        .flat_map(|region| (region.start_address..region.end_address).step_by(4))
        .collect::<BTreeSet<_>>();
    ensure!(
        reviewed_data_addresses == expected_reviewed_addresses,
        "reviewed inline-data words do not exactly match file-backed text coverage"
    );
    let references = references.into_values().collect::<Vec<_>>();
    let referenced_candidates = references
        .iter()
        .map(|reference| reference.candidate_symbol.as_str())
        .collect::<BTreeSet<_>>();
    let newly_referenced_candidates = referenced_candidates
        .iter()
        .filter(|candidate| !previously_referenced.contains(**candidate))
        .count();
    let static_pointer_load_reference_edges = references
        .iter()
        .filter(|reference| !reference.pointer_slot_addresses.is_empty())
        .count();
    let gp_derived_reference_edges = references
        .iter()
        .filter(|reference| reference.uses_gp_initial_value)
        .count();
    let reference_edges_with_more_than_two_instructions = references
        .iter()
        .filter(|reference| reference.evidence.len() > 2)
        .count();
    let references_requiring_control_flow_propagation = references
        .iter()
        .filter(|reference| reference.requires_control_flow_propagation)
        .count();
    let control_flow_boundaries = control_flow_boundaries.into_values().collect::<Vec<_>>();

    Ok(ExecutableComputedReferenceReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        gp_value: format!("{gp:#010x}"),
        summary: ExecutableComputedReferenceSummary {
            japanese_script_candidates: candidates.len(),
            text_symbol_definitions: text.symbol_definitions,
            unique_text_functions: text.symbols.len(),
            unique_typed_instruction_words: typed_instruction_addresses.len(),
            reviewed_inline_data_words: reviewed_data_addresses.len(),
            straight_line_basic_blocks,
            known_value_updates,
            direct_control_flow_edges,
            control_flow_roots,
            control_flow_reachable_instruction_words: control_flow_reachable_instruction_addresses
                .len(),
            control_flow_merge_instruction_words: control_flow_merge_instruction_addresses.len(),
            control_flow_fixed_point_state_updates,
            control_flow_conservative_boundaries: control_flow_boundaries.len(),
            candidate_reference_edges: references.len(),
            references_requiring_control_flow_propagation,
            static_pointer_load_reference_edges,
            gp_derived_reference_edges,
            reference_edges_with_more_than_two_instructions,
            referenced_candidates: referenced_candidates.len(),
            newly_referenced_candidates,
            unresolved_instruction_words: 0,
        },
        review_basis: code_data_catalog.review_basis.clone(),
        reviewed_code_data: reviewed_regions
            .into_iter()
            .map(|region| region.report)
            .collect(),
        control_flow_boundaries,
        references,
        evidence_boundary: "This survey regenerates the 137 named Japanese-script candidates and scans every aligned word in every file-backed text symbol. Exact-hash reviewed inline data is excluded; every other word must typed-decode, re-encode, and yield R5900 static semantics. It propagates exact 64-bit constants from zero, the ELF GP value, lui, selected scalar integer immediate/register/shift operations, and file-backed lw/lwu pointer reads. The straight-line pass resets at function entries, direct control targets, after delay slots, and reviewed data. A second function-local direct-control-flow pass models delay-slot execution, starts a conservative state after calls and at disconnected roots, and retains a value at a merge only when every discovered incoming path has the same value and evidence lineage. Delay-slot contexts that cannot be represented by one in-symbol instruction node are listed and restarted with unknown register state. It does not propagate across calls, resolve indirect control-flow targets, model indexed or runtime memory, prove stack or heap string construction, runtime reachability, or display.",
    })
}

fn parse_u32_address(value: &str) -> Result<u32> {
    let address = parse_u64_address(value)?;
    u32::try_from(address).with_context(|| format!("address exceeds 32 bits: {value}"))
}

fn parse_u64_address(value: &str) -> Result<u64> {
    let digits = value
        .strip_prefix("0x")
        .with_context(|| format!("address lacks 0x prefix: {value}"))?;
    u64::from_str_radix(digits, 16).with_context(|| format!("invalid address: {value}"))
}

#[cfg(test)]
#[path = "executable_computed_references/executable_computed_reference_tests.rs"]
mod executable_computed_reference_tests;
