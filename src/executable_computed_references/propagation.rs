use std::collections::BTreeSet;

#[cfg(test)]
use std::collections::BTreeMap;

use anyhow::{Context, Result};
use typed_isa_core::{ControlAction, ControlTarget, StaticSemantics};

use super::control_flow::scan_control_flow;
use super::review::{ReviewedCodeDataRegion, reviewed_region_at};
use super::value_flow::{
    RegisterState, compute_known_value, read_file_backed_u32, written_general_registers,
};
use crate::executable::validate_r5900_instruction;
use crate::executable_text::FileBackedTextSymbol;
use crate::typed_evidence::{TypedInstructionEvidence, instruction_evidence};

pub(super) struct PlacedInstruction {
    pub(super) address: u64,
    pub(super) bytes: [u8; 4],
    pub(super) instruction: r5900::Instruction,
    pub(super) semantics: r5900::InstructionSemantics,
}

#[derive(Clone, Debug)]
pub(super) struct CandidateProduction {
    pub(super) candidate_address: u32,
    pub(super) result_instruction: TypedInstructionEvidence,
    pub(super) evidence: Vec<TypedInstructionEvidence>,
    pub(super) pointer_slot_addresses: Vec<u32>,
    pub(super) uses_gp_initial_value: bool,
}

pub(super) struct TextSymbolScan {
    pub(super) productions: Vec<CandidateProduction>,
    pub(super) control_flow_productions: Vec<CandidateProduction>,
    pub(super) typed_instruction_addresses: BTreeSet<u64>,
    pub(super) reviewed_data_addresses: BTreeSet<u64>,
    pub(super) basic_blocks: usize,
    pub(super) known_value_updates: usize,
    pub(super) direct_control_flow_edges: usize,
    pub(super) control_flow_roots: usize,
    pub(super) control_flow_reachable_instruction_addresses: BTreeSet<u64>,
    pub(super) control_flow_merge_instruction_addresses: BTreeSet<u64>,
    pub(super) control_flow_fixed_point_state_updates: usize,
    pub(super) control_flow_conservative_boundaries: Vec<super::control_flow::ControlFlowBoundary>,
}

pub(super) fn scan_text_symbol(
    file: &object::File<'_>,
    symbol: &FileBackedTextSymbol,
    gp: u32,
    candidate_addresses: &BTreeSet<u32>,
    reviewed_regions: &[ReviewedCodeDataRegion],
) -> Result<TextSymbolScan> {
    let mut placed = Vec::with_capacity(symbol.bytes.len() / 4);
    let mut typed_instruction_addresses = BTreeSet::new();
    let mut reviewed_data_addresses = BTreeSet::new();
    for (index, chunk) in symbol.bytes.chunks_exact(4).enumerate() {
        let offset = index
            .checked_mul(4)
            .context("instruction offset overflow")?;
        let address = symbol.address + u64::try_from(offset)?;
        if reviewed_region_at(reviewed_regions, address).is_some() {
            reviewed_data_addresses.insert(address);
            placed.push(None);
            continue;
        }
        let bytes: [u8; 4] = chunk.try_into().expect("four-byte instruction");
        let instruction = validate_r5900_instruction(&bytes, address).with_context(|| {
            format!(
                "unreviewed text word is not canonical R5900 at {address:#010x} in {}",
                symbol.owners.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        })?;
        let location = r5900::CodeLocation::try_from(address)?;
        let semantics =
            r5900::R5900::semantics(&instruction, &r5900::AnalysisPoint::new(location))?;
        typed_instruction_addresses.insert(address);
        placed.push(Some(PlacedInstruction {
            address,
            bytes,
            instruction,
            semantics,
        }));
    }

    let block_starts = find_block_starts(symbol, &placed)?;
    let mut state = RegisterState::at_block_entry(gp);
    let mut productions = Vec::new();
    let mut known_value_updates = 0;
    for (index, placed_instruction) in placed.iter().enumerate() {
        if block_starts.contains(&index) {
            state = RegisterState::at_block_entry(gp);
        }
        let Some(placed_instruction) = placed_instruction else {
            state = RegisterState::at_block_entry(gp);
            continue;
        };
        let computed = compute_known_value(
            &state,
            &placed_instruction.instruction,
            index,
            &mut |address| read_file_backed_u32(file, address),
        )?;
        for register in written_general_registers(&placed_instruction.semantics) {
            state.clear(register);
        }
        let Some((destination, value)) = computed else {
            continue;
        };
        known_value_updates += 1;
        state.set(destination, value.clone());
        let Ok(candidate_address) = u32::try_from(value.value) else {
            continue;
        };
        if !candidate_addresses.contains(&candidate_address) {
            continue;
        }
        let evidence = value
            .instruction_indices
            .iter()
            .map(|instruction_index| {
                placed
                    .get(*instruction_index)
                    .and_then(Option::as_ref)
                    .context("constant evidence crosses reviewed inline data")
                    .map(placed_evidence)
            })
            .collect::<Result<Vec<_>>>()?;
        productions.push(CandidateProduction {
            candidate_address,
            result_instruction: placed_evidence(placed_instruction),
            evidence,
            pointer_slot_addresses: value.pointer_slot_addresses.into_iter().collect(),
            uses_gp_initial_value: value.uses_gp_initial_value,
        });
    }

    let control_flow = scan_control_flow(file, symbol, &placed, gp, candidate_addresses)?;
    Ok(TextSymbolScan {
        productions,
        control_flow_productions: control_flow.productions,
        typed_instruction_addresses,
        reviewed_data_addresses,
        basic_blocks: block_starts.len(),
        known_value_updates,
        direct_control_flow_edges: control_flow.direct_edges,
        control_flow_roots: control_flow.roots,
        control_flow_reachable_instruction_addresses: control_flow.reachable_instruction_addresses,
        control_flow_merge_instruction_addresses: control_flow.merge_instruction_addresses,
        control_flow_fixed_point_state_updates: control_flow.fixed_point_state_updates,
        control_flow_conservative_boundaries: control_flow.conservative_boundaries,
    })
}

fn find_block_starts(
    symbol: &FileBackedTextSymbol,
    placed: &[Option<PlacedInstruction>],
) -> Result<BTreeSet<usize>> {
    let mut starts = BTreeSet::from([0]);
    for (index, instruction) in placed.iter().enumerate() {
        let Some(instruction) = instruction else {
            if index + 1 < placed.len() {
                starts.insert(index + 1);
            }
            continue;
        };
        if let Some(target) = direct_control_target(&instruction.semantics.control_flow.action)
            && let Some(target_index) = address_index(symbol, u64::from(target.location))?
        {
            starts.insert(target_index);
        }
        if instruction.semantics.control_flow.delay_slot.is_some() {
            if index + 2 < placed.len() {
                starts.insert(index + 2);
            }
        } else if !matches!(
            instruction.semantics.control_flow.action,
            ControlAction::Continue
        ) && index + 1 < placed.len()
        {
            starts.insert(index + 1);
        }
    }
    Ok(starts)
}

fn direct_control_target(
    action: &ControlAction<r5900::AnalysisPoint, r5900::IndirectTarget, r5900::Boundary>,
) -> Option<r5900::AnalysisPoint> {
    match action {
        ControlAction::Transfer {
            target: ControlTarget::Direct(target),
        }
        | ControlAction::LinkedTransfer {
            target: ControlTarget::Direct(target),
            ..
        } => Some(*target),
        _ => None,
    }
}

pub(super) fn address_index(symbol: &FileBackedTextSymbol, address: u64) -> Result<Option<usize>> {
    let Some(relative) = address.checked_sub(symbol.address) else {
        return Ok(None);
    };
    if !relative.is_multiple_of(4) || relative >= u64::try_from(symbol.bytes.len())? {
        return Ok(None);
    }
    Ok(Some(usize::try_from(relative / 4)?))
}

pub(super) fn placed_evidence(placed: &PlacedInstruction) -> TypedInstructionEvidence {
    instruction_evidence(&placed.bytes, placed.address, &placed.instruction)
}

#[cfg(test)]
pub(super) fn propagate_straight_line_words(
    words: &[u32],
    gp: u32,
    memory: &BTreeMap<u32, u32>,
    candidate_addresses: &BTreeSet<u32>,
) -> Result<Vec<CandidateProduction>> {
    let placed = place_test_words(words)?;
    let mut state = RegisterState::at_block_entry(gp);
    let mut productions = Vec::new();
    for (index, instruction) in placed.iter().enumerate() {
        let computed =
            compute_known_value(&state, &instruction.instruction, index, &mut |address| {
                Ok(memory.get(&address).copied())
            })?;
        for register in written_general_registers(&instruction.semantics) {
            state.clear(register);
        }
        let Some((destination, value)) = computed else {
            continue;
        };
        state.set(destination, value.clone());
        let Ok(candidate_address) = u32::try_from(value.value) else {
            continue;
        };
        if !candidate_addresses.contains(&candidate_address) {
            continue;
        }
        let evidence = value
            .instruction_indices
            .iter()
            .map(|instruction_index| placed_evidence(&placed[*instruction_index]))
            .collect();
        productions.push(CandidateProduction {
            candidate_address,
            result_instruction: placed_evidence(instruction),
            evidence,
            pointer_slot_addresses: value.pointer_slot_addresses.into_iter().collect(),
            uses_gp_initial_value: value.uses_gp_initial_value,
        });
    }
    Ok(productions)
}

#[cfg(test)]
pub(super) fn block_starts_for_words(words: &[u32]) -> Result<BTreeSet<usize>> {
    let instructions = place_test_words(words)?;
    let placed = instructions.into_iter().map(Some).collect::<Vec<_>>();
    let symbol = FileBackedTextSymbol {
        address: 0x0040_0000,
        bytes: words.iter().flat_map(|word| word.to_le_bytes()).collect(),
        owners: BTreeSet::from(["test_function".to_string()]),
    };
    find_block_starts(&symbol, &placed)
}

#[cfg(test)]
pub(super) fn place_test_words(words: &[u32]) -> Result<Vec<PlacedInstruction>> {
    words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            let address = 0x0040_0000_u64 + u64::try_from(index.checked_mul(4).unwrap())?;
            let bytes = word.to_le_bytes();
            let instruction = validate_r5900_instruction(&bytes, address)?;
            let location = r5900::CodeLocation::try_from(address)?;
            let semantics =
                r5900::R5900::semantics(&instruction, &r5900::AnalysisPoint::new(location))?;
            Ok(PlacedInstruction {
                address,
                bytes,
                instruction,
                semantics,
            })
        })
        .collect()
}
