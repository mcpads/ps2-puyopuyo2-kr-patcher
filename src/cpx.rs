use std::collections::{BTreeSet, HashMap};

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object, ObjectSection, ObjectSymbol};
use serde::Serialize;
use sha2::{Digest, Sha256};

const CPX_MAGIC: &[u8; 8] = b"CPXa0100";
const CPX_HEADER_BYTES: usize = 16;
pub(crate) const CPX_BLOCK_ALIGNMENT: usize = 2_048;
const MAX_DECODED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Default, Serialize)]
pub struct CpxCommandCounts {
    pub short_literal: u64,
    pub extended_literal: u64,
    pub short_match_8: u64,
    pub short_match_16: u64,
    pub extended_match_8: u64,
    pub extended_match_16: u64,
    pub literal_then_match: u64,
}

impl CpxCommandCounts {
    fn add(&mut self, other: &Self) {
        self.short_literal += other.short_literal;
        self.extended_literal += other.extended_literal;
        self.short_match_8 += other.short_match_8;
        self.short_match_16 += other.short_match_16;
        self.extended_match_8 += other.extended_match_8;
        self.extended_match_16 += other.extended_match_16;
        self.literal_then_match += other.literal_then_match;
    }
}

#[derive(Clone, Debug)]
pub struct DecodedCpxBlock {
    pub bytes: Vec<u8>,
    pub encoded_byte_length: usize,
    pub header_value: u32,
    pub commands: CpxCommandCounts,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxBlockSurvey {
    pub index: usize,
    pub offset: u64,
    pub slot_byte_length: u64,
    pub encoded_byte_length: u64,
    pub padding_byte_length: u64,
    pub header_value: u32,
    pub decoded_byte_length: u64,
    pub decoded_sha256: String,
    pub decoded_prefix_hex: String,
    pub commands: CpxCommandCounts,
    pub image_record: Option<CpxImageRecord>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxImageRecord {
    pub file_list_record: usize,
    pub sector_offset: u32,
    pub sector_count: u32,
    pub stored_byte_length: u32,
    pub width: u32,
    pub height: u32,
    pub image_byte_length: u64,
    pub decoded_trailing_byte_length: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxContainerSummary {
    pub blocks: usize,
    pub cpx_region_bytes: u64,
    pub encoded_bytes: u64,
    pub padding_bytes: u64,
    pub decoded_bytes: u64,
    pub trailing_bytes: u64,
    pub unique_decoded_contents: usize,
    pub commands: CpxCommandCounts,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxTrailingRegion {
    pub offset: u64,
    pub byte_length: u64,
    pub sha256: String,
    pub prefix_hex: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxContainerSurvey {
    pub iso_path: String,
    pub byte_length: u64,
    pub sha256: String,
    pub summary: CpxContainerSummary,
    pub blocks: Vec<CpxBlockSurvey>,
    pub trailing_region: Option<CpxTrailingRegion>,
    pub file_list: Option<CpxFileListSurvey>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxFileListSurvey {
    pub symbol: String,
    pub records: usize,
    pub cpx_records: usize,
    pub non_cpx_records: usize,
    pub non_cpx_payload_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxExecutableIdentity {
    pub iso_path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxFileListValidation {
    pub discovered_image_lists: usize,
    pub linked_containers: usize,
    pub unlinked_containers: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxCorpusSummary {
    pub containers: usize,
    pub blocks: usize,
    pub cpx_region_bytes: u64,
    pub encoded_bytes: u64,
    pub padding_bytes: u64,
    pub decoded_bytes: u64,
    pub trailing_bytes: u64,
    pub unique_decoded_contents: usize,
    pub commands: CpxCommandCounts,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpxCorpusSurvey {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable: CpxExecutableIdentity,
    pub file_lists: CpxFileListValidation,
    pub summary: CpxCorpusSummary,
    pub containers: Vec<CpxContainerSurvey>,
}

pub fn decode_cpx_block(source: &[u8]) -> Result<DecodedCpxBlock> {
    ensure!(
        source.len() >= CPX_HEADER_BYTES,
        "CPX block is shorter than its 16-byte header"
    );
    ensure!(
        &source[..CPX_MAGIC.len()] == CPX_MAGIC,
        "CPX magic mismatch"
    );
    ensure!(
        source[12..CPX_HEADER_BYTES].iter().all(|byte| *byte == 0),
        "CPX reserved header bytes are nonzero"
    );
    let header_value = u32::from_le_bytes(source[8..12].try_into()?);
    ensure!(
        header_value as usize <= MAX_DECODED_BYTES,
        "CPX header value exceeds the safety limit: {header_value}"
    );

    let mut cursor = CPX_HEADER_BYTES;
    let mut output = Vec::with_capacity(header_value as usize);
    let mut commands = CpxCommandCounts::default();
    loop {
        let token_offset = cursor;
        let token = read_byte(source, &mut cursor, "command token")?;
        if token == 0 {
            break;
        }
        match token >> 4 {
            0x0 => {
                commands.short_match_8 += 1;
                let distance = read_byte(source, &mut cursor, "8-bit match distance")? as usize;
                copy_match(
                    &mut output,
                    distance,
                    usize::from(token & 0x0f) + 2,
                    token_offset,
                )?;
            }
            0x1 => {
                commands.short_match_16 += 1;
                let distance = read_u16(source, &mut cursor, "16-bit match distance")? as usize;
                copy_match(
                    &mut output,
                    distance,
                    usize::from(token & 0x0f) + 2,
                    token_offset,
                )?;
            }
            0x2 => {
                commands.extended_match_8 += 1;
                let low_length = read_byte(source, &mut cursor, "extended match length")?;
                let distance = read_byte(source, &mut cursor, "8-bit match distance")? as usize;
                let length = (usize::from(token & 0x0f) << 8) | usize::from(low_length);
                copy_match(&mut output, distance, length, token_offset)?;
            }
            0x3 => {
                commands.extended_match_16 += 1;
                let low_length = read_byte(source, &mut cursor, "extended match length")?;
                let distance = read_u16(source, &mut cursor, "16-bit match distance")? as usize;
                let length = (usize::from(token & 0x0f) << 8) | usize::from(low_length);
                copy_match(&mut output, distance, length, token_offset)?;
            }
            0x4 | 0x5 => {
                commands.short_literal += 1;
                let length = usize::from(token & 0x1f);
                copy_literals(source, &mut cursor, &mut output, length, token_offset)?;
            }
            0x6 | 0x7 => {
                commands.extended_literal += 1;
                let low_length = read_byte(source, &mut cursor, "extended literal length")?;
                let length = (usize::from(token & 0x1f) << 8) | usize::from(low_length);
                copy_literals(source, &mut cursor, &mut output, length, token_offset)?;
            }
            0x8..=0xf => {
                commands.literal_then_match += 1;
                let literal_length = usize::from((token & 0x70) >> 4);
                copy_literals(
                    source,
                    &mut cursor,
                    &mut output,
                    literal_length,
                    token_offset,
                )?;
                let distance = read_byte(source, &mut cursor, "8-bit match distance")? as usize;
                copy_match(
                    &mut output,
                    distance,
                    usize::from(token & 0x0f) + 2,
                    token_offset,
                )?;
            }
            _ => unreachable!("all four-bit command classes are covered"),
        }
    }
    Ok(DecodedCpxBlock {
        bytes: output,
        encoded_byte_length: cursor,
        header_value,
        commands,
    })
}

/// Encode one decoded image using the command grammar accepted by the game's CPX decoder.
///
/// The matcher is deterministic: it considers prior three-byte prefixes from newest to oldest,
/// prefers the longest match, and breaks ties in favor of the nearest source bytes.
pub fn encode_cpx_block(source: &[u8]) -> Result<Vec<u8>> {
    encode_cpx_block_with_header_value(source, u32::try_from(source.len())?)
}

pub(crate) fn encode_cpx_block_with_header_value(
    source: &[u8],
    header_value: u32,
) -> Result<Vec<u8>> {
    encode_with_search_limit(source, header_value, MATCH_SEARCH_LIMIT, None)
}

pub(crate) fn encode_cpx_block_to_fit(
    source: &[u8],
    header_value: u32,
    capacity: usize,
) -> Result<Vec<u8>> {
    let standard = encode_cpx_block_with_header_value(source, header_value)?;
    if standard.len() <= capacity {
        return Ok(standard);
    }
    // Tight immutable slots may require matches hidden behind the fast search
    // budget. Search the complete legal history without changing decoded pixels.
    let exhaustive = encode_with_search_limit(source, header_value, MAX_MATCH_DISTANCE, None)?;
    let mut best = if exhaustive.len() < standard.len() {
        exhaustive
    } else {
        standard
    };
    if best.len() <= capacity {
        return Ok(best);
    }
    // A longest match can hide a longer match starting one byte later. Try
    // deferring that byte before changing the immutable slot or image data.
    for minimum_gain in [2, 1] {
        let lazy =
            encode_with_search_limit(source, header_value, MAX_MATCH_DISTANCE, Some(minimum_gain))?;
        if lazy.len() < best.len() {
            best = lazy;
        }
        if best.len() <= capacity {
            break;
        }
    }
    Ok(best)
}

fn encode_with_search_limit(
    source: &[u8],
    header_value: u32,
    search_limit: usize,
    minimum_lazy_gain: Option<usize>,
) -> Result<Vec<u8>> {
    ensure!(
        source.len() <= u32::MAX as usize && source.len() <= MAX_DECODED_BYTES,
        "CPX input exceeds the supported decoded length"
    );
    ensure!(
        header_value as usize <= MAX_DECODED_BYTES,
        "CPX header value exceeds the supported decoded length"
    );

    let mut output = Vec::new();
    output.extend_from_slice(CPX_MAGIC);
    output.extend_from_slice(&header_value.to_le_bytes());
    output.extend_from_slice(&[0; 4]);

    let mut positions: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut cursor = 0_usize;
    let mut literal_start = 0_usize;
    while cursor < source.len() {
        let matched = find_match(source, cursor, &positions, search_limit);
        if let Some((distance, length)) = matched {
            if minimum_lazy_gain.is_some_and(|minimum_gain| {
                find_match(source, cursor + 1, &positions, search_limit)
                    .is_some_and(|(_, next_length)| next_length >= length + minimum_gain)
            }) {
                remember_position(source, cursor, &mut positions);
                cursor += 1;
                continue;
            }
            let literals = &source[literal_start..cursor];
            if !emit_literal_then_match(&mut output, literals, distance, length)? {
                emit_literals(&mut output, literals);
                emit_match(&mut output, distance, length)?;
            }
            let end = cursor + length;
            for position in cursor..end {
                remember_position(source, position, &mut positions);
            }
            cursor = end;
            literal_start = cursor;
        } else {
            remember_position(source, cursor, &mut positions);
            cursor += 1;
        }
    }
    emit_literals(&mut output, &source[literal_start..]);
    output.push(0);

    let decoded = decode_cpx_block(&output)?;
    ensure!(
        decoded.header_value == header_value && decoded.bytes == source,
        "CPX encoder failed its exact decode round trip"
    );
    Ok(output)
}

const MAX_MATCH_DISTANCE: usize = u16::MAX as usize;
const MAX_MATCH_LENGTH: usize = 0x0fff;
const MATCH_SEARCH_LIMIT: usize = 512;

fn find_match(
    source: &[u8],
    cursor: usize,
    positions: &HashMap<u32, Vec<usize>>,
    search_limit: usize,
) -> Option<(usize, usize)> {
    let key = prefix_key(source, cursor)?;
    let candidates = positions.get(&key)?;
    let mut best = None;
    let maximum_length = (source.len() - cursor).min(MAX_MATCH_LENGTH);
    for &candidate in candidates.iter().rev().take(search_limit) {
        let distance = cursor - candidate;
        if distance == 0 {
            continue;
        }
        if distance > MAX_MATCH_DISTANCE {
            break;
        }
        let length = cyclic_match_length(source, cursor, candidate, distance, maximum_length);
        if best.is_none_or(|(best_distance, best_length)| {
            length > best_length || (length == best_length && distance < best_distance)
        }) {
            best = Some((distance, length));
            if length == maximum_length {
                break;
            }
        }
    }
    best
}

fn cyclic_match_length(
    source: &[u8],
    cursor: usize,
    candidate: usize,
    distance: usize,
    maximum_length: usize,
) -> usize {
    let mut length = 3_usize;
    while length < maximum_length {
        let pattern_offset = length % distance;
        let chunk_length = (distance - pattern_offset).min(maximum_length - length);
        let pattern =
            &source[candidate + pattern_offset..candidate + pattern_offset + chunk_length];
        let pending = &source[cursor + length..cursor + length + chunk_length];
        if pattern == pending {
            length += chunk_length;
            continue;
        }
        length += pattern
            .iter()
            .zip(pending)
            .take_while(|(left, right)| left == right)
            .count();
        break;
    }
    length
}

fn remember_position(source: &[u8], position: usize, positions: &mut HashMap<u32, Vec<usize>>) {
    if let Some(key) = prefix_key(source, position) {
        positions.entry(key).or_default().push(position);
    }
}

fn prefix_key(source: &[u8], position: usize) -> Option<u32> {
    let bytes = source.get(position..position + 3)?;
    Some(u32::from(bytes[0]) << 16 | u32::from(bytes[1]) << 8 | u32::from(bytes[2]))
}

fn emit_literals(output: &mut Vec<u8>, mut literals: &[u8]) {
    while !literals.is_empty() {
        let length = literals.len().min(0x1fff);
        if length <= 0x1f {
            output.push(0x40 | length as u8);
        } else {
            output.push(0x60 | ((length >> 8) as u8 & 0x1f));
            output.push(length as u8);
        }
        output.extend_from_slice(&literals[..length]);
        literals = &literals[length..];
    }
}

fn emit_literal_then_match(
    output: &mut Vec<u8>,
    literals: &[u8],
    distance: usize,
    length: usize,
) -> Result<bool> {
    if literals.is_empty() || literals.len() > 7 || distance > u8::MAX as usize || length > 17 {
        return Ok(false);
    }
    ensure!(
        (3..=17).contains(&length) && distance > 0,
        "CPX combined literal-match is outside its command range"
    );
    output.push(0x80 | ((literals.len() as u8) << 4) | (length - 2) as u8);
    output.extend_from_slice(literals);
    output.push(distance as u8);
    Ok(true)
}

fn emit_match(output: &mut Vec<u8>, distance: usize, length: usize) -> Result<()> {
    ensure!(
        distance > 0 && distance <= MAX_MATCH_DISTANCE,
        "CPX match distance is out of range"
    );
    ensure!(
        (3..=MAX_MATCH_LENGTH).contains(&length),
        "CPX match length is out of range"
    );
    if length <= 17 {
        if distance <= u8::MAX as usize {
            output.push((length - 2) as u8);
            output.push(distance as u8);
        } else {
            output.push(0x10 | (length - 2) as u8);
            output.extend_from_slice(&(distance as u16).to_le_bytes());
        }
    } else if distance <= u8::MAX as usize {
        output.push(0x20 | ((length >> 8) as u8 & 0x0f));
        output.push(length as u8);
        output.push(distance as u8);
    } else {
        output.push(0x30 | ((length >> 8) as u8 & 0x0f));
        output.push(length as u8);
        output.extend_from_slice(&(distance as u16).to_le_bytes());
    }
    Ok(())
}

pub fn survey_cpx_container(iso_path: &str, source: &[u8]) -> Result<CpxContainerSurvey> {
    ensure!(!iso_path.is_empty(), "CPX ISO path is empty");
    ensure!(!source.is_empty(), "CPX container is empty: {iso_path}");
    ensure!(
        source.len().is_multiple_of(CPX_BLOCK_ALIGNMENT),
        "CPX container length is not 2048-byte aligned: {iso_path}"
    );

    let mut offset = 0_usize;
    let mut blocks = Vec::new();
    let mut decoded_hashes = BTreeSet::new();
    let mut command_totals = CpxCommandCounts::default();
    let mut encoded_bytes = 0_u64;
    let mut padding_bytes = 0_u64;
    let mut decoded_bytes = 0_u64;
    let mut trailing_region = None;
    while offset < source.len() {
        let decoded = decode_cpx_block(&source[offset..])
            .with_context(|| format!("failed to decode CPX block at {iso_path}+{offset:#x}"))?;
        let encoded_end = offset
            .checked_add(decoded.encoded_byte_length)
            .context("CPX encoded range overflow")?;
        let slot_end = align_up(encoded_end, CPX_BLOCK_ALIGNMENT)?;
        ensure!(
            slot_end <= source.len(),
            "CPX block exceeds container at {iso_path}+{offset:#x}"
        );
        ensure!(
            source[encoded_end..slot_end].iter().all(|byte| *byte == 0),
            "CPX block has nonzero alignment padding at {iso_path}+{offset:#x}"
        );
        let decoded_sha256 = format!("{:x}", Sha256::digest(&decoded.bytes));
        let padding_byte_length = slot_end - encoded_end;
        let slot_byte_length = slot_end - offset;
        encoded_bytes += decoded.encoded_byte_length as u64;
        padding_bytes += padding_byte_length as u64;
        decoded_bytes += decoded.bytes.len() as u64;
        decoded_hashes.insert(decoded_sha256.clone());
        command_totals.add(&decoded.commands);
        blocks.push(CpxBlockSurvey {
            index: blocks.len(),
            offset: offset as u64,
            slot_byte_length: slot_byte_length as u64,
            encoded_byte_length: decoded.encoded_byte_length as u64,
            padding_byte_length: padding_byte_length as u64,
            header_value: decoded.header_value,
            decoded_byte_length: decoded.bytes.len() as u64,
            decoded_sha256,
            decoded_prefix_hex: encode_hex(&decoded.bytes[..decoded.bytes.len().min(16)]),
            commands: decoded.commands,
            image_record: None,
        });
        offset = slot_end;
        if offset < source.len() && !source[offset..].starts_with(CPX_MAGIC) {
            ensure!(
                !contains_aligned_cpx(&source[offset..]),
                "CPX blocks resume after a non-CPX region in {iso_path}"
            );
            let trailing = &source[offset..];
            trailing_region = Some(CpxTrailingRegion {
                offset: offset as u64,
                byte_length: trailing.len() as u64,
                sha256: format!("{:x}", Sha256::digest(trailing)),
                prefix_hex: encode_hex(&trailing[..trailing.len().min(16)]),
            });
            break;
        }
    }

    ensure!(
        !blocks.is_empty(),
        "CPX container has no blocks: {iso_path}"
    );
    let cpx_region_bytes = trailing_region
        .as_ref()
        .map_or(source.len() as u64, |trailing| trailing.offset);
    let trailing_bytes = trailing_region
        .as_ref()
        .map_or(0, |trailing| trailing.byte_length);
    Ok(CpxContainerSurvey {
        iso_path: iso_path.to_string(),
        byte_length: source.len() as u64,
        sha256: format!("{:x}", Sha256::digest(source)),
        summary: CpxContainerSummary {
            blocks: blocks.len(),
            cpx_region_bytes,
            encoded_bytes,
            padding_bytes,
            decoded_bytes,
            trailing_bytes,
            unique_decoded_contents: decoded_hashes.len(),
            commands: command_totals,
        },
        blocks,
        trailing_region,
        file_list: None,
    })
}

pub fn assemble_cpx_corpus_survey(
    source_bin_sha256: String,
    executable: CpxExecutableIdentity,
    file_lists: CpxFileListValidation,
    containers: Vec<CpxContainerSurvey>,
) -> Result<CpxCorpusSurvey> {
    ensure!(!containers.is_empty(), "CPX corpus has no containers");
    let mut paths = BTreeSet::new();
    let mut decoded_hashes = BTreeSet::new();
    let mut commands = CpxCommandCounts::default();
    let mut blocks = 0_usize;
    let mut cpx_region_bytes = 0_u64;
    let mut encoded_bytes = 0_u64;
    let mut padding_bytes = 0_u64;
    let mut decoded_bytes = 0_u64;
    let mut trailing_bytes = 0_u64;
    for container in &containers {
        ensure!(
            paths.insert(&container.iso_path),
            "duplicate CPX container: {}",
            container.iso_path
        );
        blocks += container.summary.blocks;
        cpx_region_bytes += container.summary.cpx_region_bytes;
        encoded_bytes += container.summary.encoded_bytes;
        padding_bytes += container.summary.padding_bytes;
        decoded_bytes += container.summary.decoded_bytes;
        trailing_bytes += container.summary.trailing_bytes;
        commands.add(&container.summary.commands);
        decoded_hashes.extend(
            container
                .blocks
                .iter()
                .map(|block| block.decoded_sha256.as_str()),
        );
    }
    Ok(CpxCorpusSurvey {
        schema: 1,
        source_bin_sha256,
        executable,
        file_lists,
        summary: CpxCorpusSummary {
            containers: containers.len(),
            blocks,
            cpx_region_bytes,
            encoded_bytes,
            padding_bytes,
            decoded_bytes,
            trailing_bytes,
            unique_decoded_contents: decoded_hashes.len(),
            commands,
        },
        containers,
    })
}

pub fn link_image_file_lists(
    executable: &[u8],
    containers: &mut [CpxContainerSurvey],
) -> Result<CpxFileListValidation> {
    let file = object::File::parse(executable).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips,
        "ELF architecture is not 32-bit MIPS"
    );
    ensure!(
        file.endianness() == Endianness::Little,
        "ELF is not little-endian"
    );

    let (_, name_table) = read_symbol_bytes(&file, "FileListName")?;
    let (_, list_table) = read_symbol_bytes(&file, "FileListList")?;
    ensure!(
        name_table.len() == list_table.len() && name_table.len().is_multiple_of(4),
        "FileListName and FileListList pointer arrays differ"
    );

    let name_pointers = read_u32_table(&name_table)?;
    let list_pointers = read_u32_table(&list_table)?;
    let mut discovered_image_lists = 0_usize;
    let mut linked_containers = BTreeSet::new();
    for (&name_pointer, &list_pointer) in name_pointers.iter().zip(&list_pointers) {
        if name_pointer == 0 && list_pointer == 0 {
            continue;
        }
        ensure!(
            name_pointer != 0 && list_pointer != 0,
            "file-list pointer pair is incomplete"
        );
        let iso_path = normalize_executable_iso_path(&read_c_string(&file, name_pointer.into())?);
        if !iso_path.starts_with("/IMAGE_") {
            continue;
        }
        discovered_image_lists += 1;
        let (symbol, records) = read_symbol_at_address(&file, list_pointer.into())?;
        if let Some(container) = containers
            .iter_mut()
            .find(|container| container.iso_path == iso_path)
        {
            ensure!(
                linked_containers.insert(iso_path.clone()),
                "duplicate image file-list mapping: {iso_path}"
            );
            validate_file_list(container, &symbol, &records)?;
        }
    }

    let unlinked_containers = containers
        .iter()
        .filter(|container| container.file_list.is_none())
        .map(|container| container.iso_path.clone())
        .collect();
    Ok(CpxFileListValidation {
        discovered_image_lists,
        linked_containers: linked_containers.len(),
        unlinked_containers,
    })
}

fn validate_file_list(
    container: &mut CpxContainerSurvey,
    symbol: &str,
    source: &[u8],
) -> Result<()> {
    ensure!(
        source.len().is_multiple_of(20),
        "image file list is not a sequence of 20-byte records: {symbol}"
    );
    let records: Vec<[u32; 5]> = source
        .chunks_exact(20)
        .map(|record| {
            Ok([
                read_u32_at(record, 0)?,
                read_u32_at(record, 4)?,
                read_u32_at(record, 8)?,
                read_u32_at(record, 12)?,
                read_u32_at(record, 16)?,
            ])
        })
        .collect::<Result<_>>()?;
    ensure!(
        records.len() >= container.blocks.len(),
        "image file list has fewer records than CPX blocks: {symbol}"
    );

    let mut image_records = Vec::with_capacity(container.blocks.len());
    for (index, (block, record)) in container.blocks.iter().zip(&records).enumerate() {
        let [
            sector_offset,
            sector_count,
            stored_byte_length,
            width,
            height,
        ] = *record;
        let record_offset = u64::from(sector_offset) * CPX_BLOCK_ALIGNMENT as u64;
        let record_slot_length = u64::from(sector_count) * CPX_BLOCK_ALIGNMENT as u64;
        ensure!(
            record_offset == block.offset,
            "CPX record offset differs at {} record {index}",
            container.iso_path
        );
        ensure!(
            record_slot_length == block.slot_byte_length,
            "CPX record slot length differs at {} record {index}",
            container.iso_path
        );
        ensure!(
            u64::from(stored_byte_length) + 1 == block.encoded_byte_length,
            "CPX record stored length differs at {} record {index}",
            container.iso_path
        );
        ensure!(
            width > 0 && height > 0,
            "CPX image dimensions are zero at {} record {index}",
            container.iso_path
        );
        let image_byte_length = u64::from(width)
            .checked_mul(u64::from(height))
            .context("CPX image dimensions overflow")?;
        ensure!(
            image_byte_length == u64::from(block.header_value),
            "CPX header differs from image dimensions at {} record {index}",
            container.iso_path
        );
        let decoded_trailing_byte_length = block
            .decoded_byte_length
            .checked_sub(image_byte_length)
            .with_context(|| {
                format!(
                    "CPX output is shorter than image dimensions at {} record {index}",
                    container.iso_path
                )
            })?;
        image_records.push(CpxImageRecord {
            file_list_record: index,
            sector_offset,
            sector_count,
            stored_byte_length,
            width,
            height,
            image_byte_length,
            decoded_trailing_byte_length,
        });
    }

    let mut next_offset = container.summary.cpx_region_bytes;
    let mut non_cpx_payload_bytes = 0_u64;
    for (index, record) in records.iter().enumerate().skip(container.blocks.len()) {
        let [
            sector_offset,
            sector_count,
            stored_byte_length,
            width,
            height,
        ] = *record;
        let record_offset = u64::from(sector_offset) * CPX_BLOCK_ALIGNMENT as u64;
        let record_slot_length = u64::from(sector_count) * CPX_BLOCK_ALIGNMENT as u64;
        ensure!(
            record_offset == next_offset,
            "non-CPX record offset differs at {} record {index}",
            container.iso_path
        );
        ensure!(
            sector_count > 0 && u64::from(stored_byte_length) <= record_slot_length,
            "non-CPX record length exceeds its slot at {} record {index}",
            container.iso_path
        );
        ensure!(
            width == 0 && height == 0,
            "non-CPX record declares image dimensions at {} record {index}",
            container.iso_path
        );
        next_offset = next_offset
            .checked_add(record_slot_length)
            .context("image file-list range overflow")?;
        non_cpx_payload_bytes += u64::from(stored_byte_length);
    }
    ensure!(
        next_offset == container.byte_length,
        "image file list does not cover its whole container: {}",
        container.iso_path
    );

    for (block, image_record) in container.blocks.iter_mut().zip(image_records) {
        block.image_record = Some(image_record);
    }
    container.file_list = Some(CpxFileListSurvey {
        symbol: symbol.to_string(),
        records: records.len(),
        cpx_records: container.blocks.len(),
        non_cpx_records: records.len() - container.blocks.len(),
        non_cpx_payload_bytes,
    });
    Ok(())
}

pub(crate) fn read_symbol_bytes(file: &object::File<'_>, name: &str) -> Result<(u64, Vec<u8>)> {
    let symbol = file
        .symbols()
        .find(|symbol| symbol.name().is_ok_and(|candidate| candidate == name))
        .with_context(|| format!("ELF symbol not found: {name}"))?;
    ensure!(symbol.is_definition(), "ELF symbol is not defined: {name}");
    let bytes = read_address_range(file, symbol.address(), symbol.size())?;
    Ok((symbol.address(), bytes))
}

fn read_symbol_at_address(file: &object::File<'_>, address: u64) -> Result<(String, Vec<u8>)> {
    let symbol = file
        .symbols()
        .find(|symbol| symbol.is_definition() && symbol.address() == address && symbol.size() > 0)
        .with_context(|| format!("ELF symbol not found at {address:#x}"))?;
    let name = symbol
        .name()
        .context("ELF symbol name is invalid")?
        .to_string();
    let bytes = read_address_range(file, address, symbol.size())?;
    Ok((name, bytes))
}

pub(crate) fn read_address_range(
    file: &object::File<'_>,
    address: u64,
    length: u64,
) -> Result<Vec<u8>> {
    ensure!(length > 0, "ELF address range is empty at {address:#x}");
    let section = file
        .sections()
        .find(|section| {
            address >= section.address()
                && address
                    .checked_add(length)
                    .is_some_and(|end| end <= section.address().saturating_add(section.size()))
        })
        .with_context(|| format!("ELF address range has no section: {address:#x}+{length:#x}"))?;
    let offset = usize::try_from(address - section.address()).context("ELF offset is too large")?;
    let length = usize::try_from(length).context("ELF range is too large")?;
    let end = offset.checked_add(length).context("ELF range overflow")?;
    Ok(section
        .data()?
        .get(offset..end)
        .with_context(|| format!("ELF address range exceeds section: {address:#x}"))?
        .to_vec())
}

fn read_c_string(file: &object::File<'_>, address: u64) -> Result<String> {
    let section = file
        .sections()
        .find(|section| {
            address >= section.address()
                && address < section.address().saturating_add(section.size())
        })
        .with_context(|| format!("ELF string address has no section: {address:#x}"))?;
    let offset = usize::try_from(address - section.address()).context("ELF offset is too large")?;
    let remainder = section
        .data()?
        .get(offset..)
        .with_context(|| format!("ELF string address exceeds section: {address:#x}"))?;
    let end = remainder
        .iter()
        .position(|byte| *byte == 0)
        .with_context(|| format!("ELF string is not terminated: {address:#x}"))?;
    Ok(std::str::from_utf8(&remainder[..end])
        .context("ELF file-list name is not UTF-8")?
        .to_string())
}

fn read_u32_table(source: &[u8]) -> Result<Vec<u32>> {
    ensure!(source.len().is_multiple_of(4), "u32 table is misaligned");
    source
        .chunks_exact(4)
        .map(|bytes| Ok(u32::from_le_bytes(bytes.try_into()?)))
        .collect()
}

pub(crate) fn read_u32_at(source: &[u8], offset: usize) -> Result<u32> {
    let bytes = source
        .get(offset..offset + 4)
        .with_context(|| format!("u32 field exceeds record at offset {offset}"))?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

fn normalize_executable_iso_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn read_byte(source: &[u8], cursor: &mut usize, label: &str) -> Result<u8> {
    let byte = source
        .get(*cursor)
        .copied()
        .with_context(|| format!("CPX stream ended while reading {label} at {:#x}", *cursor))?;
    *cursor += 1;
    Ok(byte)
}

fn read_u16(source: &[u8], cursor: &mut usize, label: &str) -> Result<u16> {
    let low = read_byte(source, cursor, label)?;
    let high = read_byte(source, cursor, label)?;
    Ok(u16::from_le_bytes([low, high]))
}

fn copy_literals(
    source: &[u8],
    cursor: &mut usize,
    output: &mut Vec<u8>,
    length: usize,
    token_offset: usize,
) -> Result<()> {
    require_output_room(output, length, token_offset)?;
    let end = cursor
        .checked_add(length)
        .context("CPX literal range overflow")?;
    let literals = source
        .get(*cursor..end)
        .with_context(|| format!("CPX literal exceeds input at token {token_offset:#x}"))?;
    output.extend_from_slice(literals);
    *cursor = end;
    Ok(())
}

fn copy_match(
    output: &mut Vec<u8>,
    distance: usize,
    length: usize,
    token_offset: usize,
) -> Result<()> {
    require_output_room(output, length, token_offset)?;
    if length == 0 {
        return Ok(());
    }
    ensure!(
        distance > 0 && distance <= output.len(),
        "CPX match distance {distance} has no decoded history at token {token_offset:#x}"
    );
    for _ in 0..length {
        let source_index = output.len() - distance;
        output.push(output[source_index]);
    }
    Ok(())
}

fn require_output_room(output: &[u8], length: usize, token_offset: usize) -> Result<()> {
    let end = output
        .len()
        .checked_add(length)
        .context("CPX decoded range overflow")?;
    ensure!(
        end <= MAX_DECODED_BYTES,
        "CPX command exceeds the decoded safety limit at token {token_offset:#x}"
    );
    Ok(())
}

fn align_up(value: usize, alignment: usize) -> Result<usize> {
    let remainder = value % alignment;
    if remainder == 0 {
        Ok(value)
    } else {
        value
            .checked_add(alignment - remainder)
            .context("aligned CPX range overflow")
    }
}

fn contains_aligned_cpx(source: &[u8]) -> bool {
    source
        .chunks(CPX_BLOCK_ALIGNMENT)
        .any(|block| block.starts_with(CPX_MAGIC))
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests;
