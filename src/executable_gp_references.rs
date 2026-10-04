use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object, ObjectSection};
use r5900::{GeneralRegister, InstructionForm, Operands};
use serde::Serialize;

use crate::executable::validate_r5900_instruction;
use crate::executable_string_census::{
    ExecutableStringCensusInputs, ExecutableStringReferenceKind, ExecutableStringReviewCatalog,
    census_executable_strings,
};

pub struct ExecutableGpReferenceInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableGpReferenceReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub gp: ExecutableGpIdentity,
    pub summary: ExecutableGpReferenceSummary,
    pub references: Vec<ExecutableGpReference>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableGpIdentity {
    pub source_section: &'static str,
    pub source_file_offset: String,
    pub source_sha256: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableGpReferenceSummary {
    pub japanese_script_candidates: usize,
    pub text_symbol_definitions: usize,
    pub unique_text_words: usize,
    pub gp_relative_instruction_candidates: usize,
    pub typed_gp_relative_instructions: usize,
    pub direct_gp_address_references: usize,
    pub gp_pointer_load_references: usize,
    pub newly_referenced_candidates: usize,
    pub unresolved_references: usize,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ExecutableGpReference {
    pub owner_symbol: String,
    pub address: String,
    pub bytes: String,
    pub canonical: String,
    pub candidate_symbol: String,
    pub candidate_address: String,
    pub kind: ExecutableGpReferenceKind,
    pub pointer_slot_address: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutableGpReferenceKind {
    DirectAddress,
    PointerLoad,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolvedGpReference {
    candidate_address: u32,
    kind: ExecutableGpReferenceKind,
    pointer_slot_address: Option<u32>,
}

pub fn survey_executable_gp_references(
    inputs: ExecutableGpReferenceInputs<'_>,
    catalog: &ExecutableStringReviewCatalog,
) -> Result<ExecutableGpReferenceReport> {
    ensure!(
        crate::indexed_texture::sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    let file = object::File::parse(inputs.executable)
        .context("failed to parse executable for GP-reference survey")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips && file.endianness() == Endianness::Little,
        "GP-reference survey requires a little-endian MIPS ELF"
    );
    let census = census_executable_strings(
        ExecutableStringCensusInputs {
            source_bin_sha256: inputs.source_bin_sha256.clone(),
            executable_iso_path: inputs.executable_iso_path.clone(),
            executable_sha256: inputs.executable_sha256.clone(),
            executable: inputs.executable,
        },
        catalog,
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
    let gp_identity = crate::executable_text::read_elf_gp_identity(&file)?;
    let gp = gp_identity.value;
    let main = file
        .section_by_name("main")
        .context("executable has no main section")?;
    let main_data = main.data()?;
    let text = crate::executable_text::collect_file_backed_text(&file)?;
    let text_symbol_definitions = text.symbol_definitions;
    let text_words = text.unique_words()?;
    let mut references = BTreeSet::new();
    let mut gp_relative_instruction_candidates = 0;
    let mut typed_gp_relative_instructions = 0;
    for word in text_words.values() {
        if !is_gp_reference_candidate(&word.bytes) {
            continue;
        }
        gp_relative_instruction_candidates += 1;
        let instruction = validate_r5900_instruction(&word.bytes, word.address)?;
        typed_gp_relative_instructions += 1;
        let Some(resolved) =
            inspect_gp_reference(&instruction, gp, main.address(), main_data, &candidates)?
        else {
            continue;
        };
        let candidate_symbol = candidates
            .get(&resolved.candidate_address)
            .expect("resolved GP candidate is declared");
        for owner_symbol in &word.owners {
            references.insert(ExecutableGpReference {
                owner_symbol: owner_symbol.clone(),
                address: format!("{:#010x}", word.address),
                bytes: word
                    .bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                canonical: r5900::format_instruction(&instruction),
                candidate_symbol: candidate_symbol.clone(),
                candidate_address: format!("{:#010x}", resolved.candidate_address),
                kind: resolved.kind,
                pointer_slot_address: resolved
                    .pointer_slot_address
                    .map(|address| format!("{address:#010x}")),
            });
        }
    }
    let references = references.into_iter().collect::<Vec<_>>();
    let direct_gp_address_references = references
        .iter()
        .filter(|reference| reference.kind == ExecutableGpReferenceKind::DirectAddress)
        .count();
    let gp_pointer_load_references = references.len() - direct_gp_address_references;
    let gp_candidate_symbols = references
        .iter()
        .map(|reference| reference.candidate_symbol.as_str())
        .collect::<BTreeSet<_>>();
    let previously_referenced_symbols = census
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
        .map(|candidate| candidate.symbol.as_str())
        .collect::<BTreeSet<_>>();
    let newly_referenced_candidates = gp_candidate_symbols
        .difference(&previously_referenced_symbols)
        .count();

    Ok(ExecutableGpReferenceReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        gp: ExecutableGpIdentity {
            source_section: ".reginfo",
            source_file_offset: format!("{:#010x}", gp_identity.file_offset),
            source_sha256: gp_identity.source_sha256,
            value: format!("{gp:#010x}"),
        },
        summary: ExecutableGpReferenceSummary {
            japanese_script_candidates: candidates.len(),
            text_symbol_definitions,
            unique_text_words: text_words.len(),
            gp_relative_instruction_candidates,
            typed_gp_relative_instructions,
            direct_gp_address_references,
            gp_pointer_load_references,
            newly_referenced_candidates,
            unresolved_references: 0,
        },
        references,
        evidence_boundary: "This survey regenerates the 137 named Japanese-script candidates, reads the ELF .reginfo GP value, and scans every aligned word covered by a file-backed text symbol. Raw opcode and GP-register operands select addiu/daddiu and lw/lwu candidates; every selected word is typed-decoded, re-encoded, and given static semantics before resolution. It resolves direct GP addresses and one 32-bit pointer slot inside the file-backed main section. It does not treat other text-tagged words as R5900 instructions or perform arbitrary register-value propagation, indexed table analysis, stack or heap string construction, runtime reachability, or display verification.",
    })
}

fn is_gp_reference_candidate(bytes: &[u8; 4]) -> bool {
    let word = u32::from_le_bytes(*bytes);
    let opcode = word >> 26;
    let base = (word >> 21) & 0x1f;
    base == u32::from(GeneralRegister::GP.index()) && matches!(opcode, 0x09 | 0x19 | 0x23 | 0x27)
}

fn inspect_gp_reference(
    instruction: &r5900::Instruction,
    gp: u32,
    main_address: u64,
    main_data: &[u8],
    candidates: &BTreeMap<u32, String>,
) -> Result<Option<ResolvedGpReference>> {
    if matches!(
        instruction.form(),
        InstructionForm::Addiu | InstructionForm::Daddiu
    ) && let Operands::SignedImmediate {
        source, immediate, ..
    } = instruction.operands()
        && *source == GeneralRegister::GP
    {
        let candidate = gp.wrapping_add_signed(i32::from(*immediate));
        if candidates.contains_key(&candidate) {
            return Ok(Some(ResolvedGpReference {
                candidate_address: candidate,
                kind: ExecutableGpReferenceKind::DirectAddress,
                pointer_slot_address: None,
            }));
        }
    }
    if matches!(
        instruction.form(),
        InstructionForm::Lw | InstructionForm::Lwu
    ) && let Operands::MemoryGeneral { base, offset, .. } = instruction.operands()
        && *base == GeneralRegister::GP
    {
        let pointer_slot = gp.wrapping_add_signed(i32::from(*offset));
        let Some(relative) = u64::from(pointer_slot).checked_sub(main_address) else {
            return Ok(None);
        };
        let start = usize::try_from(relative)?;
        let Some(end) = start.checked_add(4) else {
            return Ok(None);
        };
        let Some(bytes) = main_data.get(start..end) else {
            return Ok(None);
        };
        let candidate = u32::from_le_bytes(bytes.try_into().expect("four-byte pointer slot"));
        if candidates.contains_key(&candidate) {
            return Ok(Some(ResolvedGpReference {
                candidate_address: candidate,
                kind: ExecutableGpReferenceKind::PointerLoad,
                pointer_slot_address: Some(pointer_slot),
            }));
        }
    }
    Ok(None)
}

fn parse_u32_address(value: &str) -> Result<u32> {
    let digits = value
        .strip_prefix("0x")
        .with_context(|| format!("address lacks 0x prefix: {value}"))?;
    u32::from_str_radix(digits, 16).with_context(|| format!("invalid address: {value}"))
}

#[cfg(test)]
#[path = "executable_gp_references/executable_gp_reference_tests.rs"]
mod executable_gp_reference_tests;
