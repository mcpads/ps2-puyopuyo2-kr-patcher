use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use typed_isa_core::{ControlAction, ControlTarget, DelaySlotExecution};

use super::super::propagation::{PlacedInstruction, address_index};
use super::ControlFlowBoundary;
use crate::executable_text::FileBackedTextSymbol;

pub(super) struct DirectInstructionFlow {
    pub(super) successors: Vec<BTreeSet<usize>>,
    pub(super) roots: BTreeSet<usize>,
    pub(super) unknown_roots: BTreeSet<usize>,
    pub(super) predecessor_counts: Vec<usize>,
    pub(super) conservative_boundaries: Vec<ControlFlowBoundary>,
}

pub(super) fn build_direct_instruction_flow(
    symbol: &FileBackedTextSymbol,
    placed: &[Option<PlacedInstruction>],
) -> Result<DirectInstructionFlow> {
    let mut successors = vec![BTreeSet::new(); placed.len()];
    let mut roots = BTreeSet::new();
    let mut unknown_roots = BTreeSet::new();
    let mut delay_owners = vec![None; placed.len()];
    let mut external_delay_slots = vec![false; placed.len()];
    let mut conservative_boundaries = Vec::new();

    for (index, instruction) in placed.iter().enumerate() {
        let Some(instruction) = instruction else {
            continue;
        };
        let Some(delay_slot) = &instruction.semantics.control_flow.delay_slot else {
            continue;
        };
        let Some(slot_index) = point_index(symbol, delay_slot.point)? else {
            external_delay_slots[index] = true;
            conservative_boundaries.push(ControlFlowBoundary {
                instruction_address: instruction.address,
                delay_slot_address: u64::from(delay_slot.point.location),
                boundary_kind: "delay_slot_outside_symbol",
            });
            continue;
        };
        ensure!(
            slot_index == index + 1,
            "R5900 delay slot is not the following instruction"
        );
        ensure!(
            placed[slot_index].is_some(),
            "R5900 delay slot overlaps reviewed inline data"
        );
        ensure!(
            delay_owners[slot_index].replace(index).is_none(),
            "instruction is the delay slot of multiple transfers"
        );
    }

    for (index, instruction) in placed.iter().enumerate() {
        let Some(instruction) = instruction else {
            continue;
        };
        if delay_owners[index].is_some() {
            continue;
        }
        let control = &instruction.semantics.control_flow;
        if external_delay_slots[index] {
            insert_optional(
                &mut unknown_roots,
                non_call_direct_target(symbol, &control.action)?,
            );
            add_call_return_unknown_root(symbol, &control.action, &mut unknown_roots)?;
            continue;
        }
        if let Some(delay_slot) = &control.delay_slot {
            let slot_index = point_index(symbol, delay_slot.point)?
                .context("delay slot lies outside its file-backed text symbol")?;
            ensure!(
                placed[slot_index].as_ref().is_some_and(|slot| {
                    slot.semantics.control_flow.delay_slot.is_none()
                        && !matches!(
                            slot.semantics.control_flow.action,
                            ControlAction::Transfer { .. }
                                | ControlAction::LinkedTransfer { .. }
                                | ControlAction::Return { .. }
                                | ControlAction::ExceptionReturn { .. }
                        )
                }),
                "nested control transfer in an R5900 delay slot is unsupported"
            );
            let transfer = non_call_direct_target(symbol, &control.action)?;
            let fallthrough = optional_point_index(symbol, control.fallthrough)?;
            add_call_return_unknown_root(symbol, &control.action, &mut unknown_roots)?;
            match delay_slot.execution {
                DelaySlotExecution::Always => {
                    successors[index].insert(slot_index);
                    insert_optional(&mut successors[slot_index], transfer);
                    insert_optional(&mut successors[slot_index], fallthrough);
                }
                DelaySlotExecution::WhenTransferOccurs => {
                    successors[index].insert(slot_index);
                    insert_optional(&mut successors[index], fallthrough);
                    insert_optional(&mut successors[slot_index], transfer);
                }
                DelaySlotExecution::WhenFallthroughOccurs => {
                    insert_optional(&mut successors[index], transfer);
                    successors[index].insert(slot_index);
                    insert_optional(&mut successors[slot_index], fallthrough);
                }
            }
            continue;
        }

        insert_optional(
            &mut successors[index],
            non_call_direct_target(symbol, &control.action)?,
        );
        insert_optional(
            &mut successors[index],
            optional_point_index(symbol, control.fallthrough)?,
        );
        add_call_return_unknown_root(symbol, &control.action, &mut unknown_roots)?;
    }

    split_shared_delay_slot_contexts(
        symbol,
        placed,
        &delay_owners,
        &mut successors,
        &mut unknown_roots,
        &mut conservative_boundaries,
    )?;
    let mut predecessor_counts = vec![0; placed.len()];
    for targets in &successors {
        for target in targets {
            predecessor_counts[*target] += 1;
        }
    }
    if let Some(entry) = placed.iter().position(Option::is_some) {
        roots.insert(entry);
    }
    for (index, instruction) in placed.iter().enumerate() {
        if instruction.is_some() && predecessor_counts[index] == 0 {
            roots.insert(index);
        }
    }
    Ok(DirectInstructionFlow {
        successors,
        roots,
        unknown_roots,
        predecessor_counts,
        conservative_boundaries,
    })
}

fn split_shared_delay_slot_contexts(
    symbol: &FileBackedTextSymbol,
    placed: &[Option<PlacedInstruction>],
    delay_owners: &[Option<usize>],
    successors: &mut [BTreeSet<usize>],
    unknown_roots: &mut BTreeSet<usize>,
    conservative_boundaries: &mut Vec<ControlFlowBoundary>,
) -> Result<()> {
    let mut direct_delay_entries = Vec::new();
    for (source, targets) in successors.iter().enumerate() {
        for target in targets {
            if let Some(owner) = delay_owners[*target]
                && owner != source
            {
                direct_delay_entries.push((source, *target));
            }
        }
    }
    let shared_delay_slots = direct_delay_entries
        .iter()
        .map(|(_, target)| *target)
        .collect::<BTreeSet<_>>();
    for (source, target) in direct_delay_entries {
        successors[source].remove(&target);
        let control_source = delay_owners[source].unwrap_or(source);
        conservative_boundaries.push(ControlFlowBoundary {
            instruction_address: placed[control_source]
                .as_ref()
                .context("delay-slot entry source is reviewed data")?
                .address,
            delay_slot_address: placed[target]
                .as_ref()
                .context("delay-slot entry target is reviewed data")?
                .address,
            boundary_kind: "direct_control_target_is_delay_slot",
        });
    }
    for slot in shared_delay_slots {
        unknown_roots.insert(slot);
        unknown_roots.extend(std::mem::take(&mut successors[slot]));
        if let Some(instruction) = &placed[slot] {
            insert_optional(
                unknown_roots,
                optional_point_index(symbol, instruction.semantics.control_flow.fallthrough)?,
            );
        }
    }
    Ok(())
}

fn non_call_direct_target(
    symbol: &FileBackedTextSymbol,
    action: &ControlAction<r5900::AnalysisPoint, r5900::IndirectTarget, r5900::Boundary>,
) -> Result<Option<usize>> {
    let ControlAction::Transfer {
        target: ControlTarget::Direct(target),
    } = action
    else {
        return Ok(None);
    };
    point_index(symbol, *target)
}

fn add_call_return_unknown_root(
    symbol: &FileBackedTextSymbol,
    action: &ControlAction<r5900::AnalysisPoint, r5900::IndirectTarget, r5900::Boundary>,
    roots: &mut BTreeSet<usize>,
) -> Result<()> {
    let ControlAction::LinkedTransfer { return_site, .. } = action else {
        return Ok(());
    };
    insert_optional(roots, point_index(symbol, *return_site)?);
    Ok(())
}

fn optional_point_index(
    symbol: &FileBackedTextSymbol,
    point: Option<r5900::AnalysisPoint>,
) -> Result<Option<usize>> {
    point.map_or(Ok(None), |point| point_index(symbol, point))
}

fn point_index(
    symbol: &FileBackedTextSymbol,
    point: r5900::AnalysisPoint,
) -> Result<Option<usize>> {
    address_index(symbol, u64::from(point.location))
}

fn insert_optional(targets: &mut BTreeSet<usize>, target: Option<usize>) {
    if let Some(target) = target {
        targets.insert(target);
    }
}
