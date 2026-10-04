use std::collections::{BTreeSet, VecDeque};

#[cfg(test)]
use std::collections::BTreeMap;

use anyhow::{Context, Result};

mod graph;

use graph::build_direct_instruction_flow;

#[cfg(test)]
use super::propagation::place_test_words;
use super::propagation::{CandidateProduction, PlacedInstruction, placed_evidence};
use super::value_flow::{
    KnownValue, RegisterState, compute_known_value, read_file_backed_u32, written_general_registers,
};
use crate::executable_text::FileBackedTextSymbol;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ControlFlowBoundary {
    pub(super) instruction_address: u64,
    pub(super) delay_slot_address: u64,
    pub(super) boundary_kind: &'static str,
}

pub(super) struct ControlFlowScan {
    pub(super) productions: Vec<CandidateProduction>,
    pub(super) direct_edges: usize,
    pub(super) roots: usize,
    pub(super) reachable_instruction_addresses: BTreeSet<u64>,
    pub(super) merge_instruction_addresses: BTreeSet<u64>,
    pub(super) fixed_point_state_updates: usize,
    pub(super) conservative_boundaries: Vec<ControlFlowBoundary>,
}

pub(super) fn scan_control_flow(
    file: &object::File<'_>,
    symbol: &FileBackedTextSymbol,
    placed: &[Option<PlacedInstruction>],
    gp: u32,
    candidate_addresses: &BTreeSet<u32>,
) -> Result<ControlFlowScan> {
    propagate_control_flow(symbol, placed, gp, candidate_addresses, &mut |address| {
        read_file_backed_u32(file, address)
    })
}

fn propagate_control_flow(
    symbol: &FileBackedTextSymbol,
    placed: &[Option<PlacedInstruction>],
    gp: u32,
    candidate_addresses: &BTreeSet<u32>,
    read_u32: &mut impl FnMut(u32) -> Result<Option<u32>>,
) -> Result<ControlFlowScan> {
    let flow = build_direct_instruction_flow(symbol, placed)?;
    let mut inputs = vec![None::<RegisterState>; placed.len()];
    let mut pending = VecDeque::new();
    let entry_state = RegisterState::at_block_entry(gp);
    let mut fixed_point_state_updates = 0;
    for root in &flow.roots {
        if merge_input_state(&mut inputs[*root], &entry_state) {
            pending.push_back(*root);
            fixed_point_state_updates += 1;
        }
    }
    let unknown_state = RegisterState::unknown();
    for root in &flow.unknown_roots {
        if merge_input_state(&mut inputs[*root], &unknown_state) {
            pending.push_back(*root);
            fixed_point_state_updates += 1;
        }
    }

    while let Some(index) = pending.pop_front() {
        let Some(instruction) = placed[index].as_ref() else {
            continue;
        };
        let input = inputs[index]
            .as_ref()
            .context("reachable instruction has no input state")?;
        let (output, _) = advance_state(input, instruction, index, read_u32)?;
        for successor in &flow.successors[index] {
            if merge_input_state(&mut inputs[*successor], &output) {
                pending.push_back(*successor);
                fixed_point_state_updates += 1;
            }
        }
    }

    let mut productions = Vec::new();
    let mut reachable_instruction_addresses = BTreeSet::new();
    let mut merge_instruction_addresses = BTreeSet::new();
    for (index, input) in inputs.iter().enumerate() {
        let (Some(input), Some(instruction)) = (input.as_ref(), placed[index].as_ref()) else {
            continue;
        };
        reachable_instruction_addresses.insert(instruction.address);
        if flow.predecessor_counts[index] > 1 {
            merge_instruction_addresses.insert(instruction.address);
        }
        let (_, computed) = advance_state(input, instruction, index, read_u32)?;
        let Some((_, value)) = computed else {
            continue;
        };
        let Ok(candidate_address) = u32::try_from(value.value) else {
            continue;
        };
        if !candidate_addresses.contains(&candidate_address) {
            continue;
        }
        productions.push(candidate_production(
            candidate_address,
            instruction,
            value,
            placed,
        )?);
    }

    Ok(ControlFlowScan {
        productions,
        direct_edges: flow.successors.iter().map(BTreeSet::len).sum(),
        roots: flow.roots.union(&flow.unknown_roots).count(),
        reachable_instruction_addresses,
        merge_instruction_addresses,
        fixed_point_state_updates,
        conservative_boundaries: flow.conservative_boundaries,
    })
}

fn advance_state(
    input: &RegisterState,
    instruction: &PlacedInstruction,
    index: usize,
    read_u32: &mut impl FnMut(u32) -> Result<Option<u32>>,
) -> Result<(RegisterState, Option<(r5900::GeneralRegister, KnownValue)>)> {
    let computed = compute_known_value(input, &instruction.instruction, index, read_u32)?;
    let mut output = input.clone();
    for register in written_general_registers(&instruction.semantics) {
        output.clear(register);
    }
    if let Some((destination, value)) = &computed {
        output.set(*destination, value.clone());
    }
    Ok((output, computed))
}

fn merge_input_state(slot: &mut Option<RegisterState>, incoming: &RegisterState) -> bool {
    match slot {
        Some(current) => current.retain_exact_values_from(incoming),
        None => {
            *slot = Some(incoming.clone());
            true
        }
    }
}

fn candidate_production(
    candidate_address: u32,
    instruction: &PlacedInstruction,
    value: KnownValue,
    placed: &[Option<PlacedInstruction>],
) -> Result<CandidateProduction> {
    let evidence = value
        .instruction_indices
        .iter()
        .map(|instruction_index| {
            placed
                .get(*instruction_index)
                .and_then(Option::as_ref)
                .context("control-flow evidence crosses reviewed inline data")
                .map(placed_evidence)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CandidateProduction {
        candidate_address,
        result_instruction: placed_evidence(instruction),
        evidence,
        pointer_slot_addresses: value.pointer_slot_addresses.into_iter().collect(),
        uses_gp_initial_value: value.uses_gp_initial_value,
    })
}

#[cfg(test)]
pub(super) fn propagate_control_flow_words(
    words: &[u32],
    gp: u32,
    memory: &BTreeMap<u32, u32>,
    candidate_addresses: &BTreeSet<u32>,
) -> Result<Vec<CandidateProduction>> {
    Ok(test_control_flow_scan(words, gp, memory, candidate_addresses)?.productions)
}

#[cfg(test)]
pub(super) fn control_flow_boundaries_for_words(words: &[u32]) -> Result<Vec<ControlFlowBoundary>> {
    Ok(
        test_control_flow_scan(words, 0x004f_8370, &BTreeMap::new(), &BTreeSet::new())?
            .conservative_boundaries,
    )
}

#[cfg(test)]
fn test_control_flow_scan(
    words: &[u32],
    gp: u32,
    memory: &BTreeMap<u32, u32>,
    candidate_addresses: &BTreeSet<u32>,
) -> Result<ControlFlowScan> {
    let symbol = FileBackedTextSymbol {
        address: 0x0040_0000,
        bytes: words.iter().flat_map(|word| word.to_le_bytes()).collect(),
        owners: BTreeSet::from(["test_function".to_string()]),
    };
    let placed = place_test_words(words)?
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    propagate_control_flow(&symbol, &placed, gp, candidate_addresses, &mut |address| {
        Ok(memory.get(&address).copied())
    })
}
