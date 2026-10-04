use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection, ObjectSymbol};

use super::{ExecutableCodeDataReviewCatalog, ExecutableCodeDataReviewedRegion};

#[derive(Clone, Debug)]
pub(super) struct ReviewedCodeDataRegion {
    pub(super) report: ExecutableCodeDataReviewedRegion,
    pub(super) start_address: u64,
    pub(super) end_address: u64,
}

pub(super) fn validate_code_data_review(
    file: &object::File<'_>,
    executable_sha256: &str,
    catalog: &ExecutableCodeDataReviewCatalog,
) -> Result<Vec<ReviewedCodeDataRegion>> {
    ensure!(catalog.schema == 1, "unsupported code-data review schema");
    ensure!(
        catalog.executable_sha256 == executable_sha256,
        "code-data review executable hash changed"
    );
    ensure!(
        !catalog.review_basis.trim().is_empty(),
        "code-data review basis is empty"
    );
    ensure!(
        !catalog.regions.is_empty(),
        "code-data review has no regions"
    );
    let mut reviewed = Vec::with_capacity(catalog.regions.len());
    for region in &catalog.regions {
        ensure!(!region.region_id.is_empty(), "code-data region id is empty");
        ensure!(
            !region.symbol_name.is_empty(),
            "code-data symbol name is empty"
        );
        ensure!(
            !region.note.trim().is_empty(),
            "code-data review note is empty"
        );
        let start_address = parse_address(&region.start_address)?;
        let end_address = parse_address(&region.end_address)?;
        ensure!(
            start_address.is_multiple_of(4)
                && end_address.is_multiple_of(4)
                && start_address < end_address,
            "code-data region is not a nonempty aligned range: {}",
            region.region_id
        );
        let bytes = read_file_backed_range(file, start_address, end_address)?;
        let symbol = file
            .symbols()
            .find(|symbol| symbol.name().is_ok_and(|name| name == region.symbol_name))
            .with_context(|| format!("code-data symbol is missing: {}", region.symbol_name))?;
        ensure!(
            symbol.section_index().is_some(),
            "code-data symbol has no section: {}",
            region.symbol_name
        );
        ensure!(
            symbol.address() == start_address,
            "code-data symbol address changed: {} expected {start_address:#010x}, found {:#010x}",
            region.symbol_name,
            symbol.address()
        );
        ensure!(
            crate::indexed_texture::sha256(&bytes) == region.source_sha256,
            "code-data region hash changed: {}",
            region.region_id
        );
        reviewed.push(ReviewedCodeDataRegion {
            report: ExecutableCodeDataReviewedRegion {
                region_id: region.region_id.clone(),
                symbol_name: region.symbol_name.clone(),
                start_address: region.start_address.clone(),
                end_address: region.end_address.clone(),
                byte_length: bytes.len(),
                source_sha256: region.source_sha256.clone(),
                classification: region.classification,
                note: region.note.clone(),
            },
            start_address,
            end_address,
        });
    }
    reviewed.sort_by_key(|region| region.start_address);
    for pair in reviewed.windows(2) {
        ensure!(
            pair[0].end_address <= pair[1].start_address,
            "code-data review regions overlap"
        );
    }
    Ok(reviewed)
}

pub(super) fn reviewed_region_at(
    regions: &[ReviewedCodeDataRegion],
    address: u64,
) -> Option<&ReviewedCodeDataRegion> {
    regions
        .iter()
        .find(|region| address >= region.start_address && address < region.end_address)
}

fn read_file_backed_range(
    file: &object::File<'_>,
    start_address: u64,
    end_address: u64,
) -> Result<Vec<u8>> {
    for section in file.sections() {
        let Some(relative) = start_address.checked_sub(section.address()) else {
            continue;
        };
        let byte_length = end_address
            .checked_sub(start_address)
            .context("code-data range underflow")?;
        let Some(relative_end) = relative.checked_add(byte_length) else {
            continue;
        };
        let Some((_, file_size)) = section.file_range() else {
            continue;
        };
        if relative_end > file_size {
            continue;
        }
        let start = usize::try_from(relative)?;
        let end = usize::try_from(relative_end)?;
        return section
            .data()?
            .get(start..end)
            .map(<[u8]>::to_vec)
            .context("code-data range exceeds its section");
    }
    anyhow::bail!("code-data range is not file-backed: {start_address:#010x}..{end_address:#010x}")
}

fn parse_address(value: &str) -> Result<u64> {
    let digits = value
        .strip_prefix("0x")
        .with_context(|| format!("address lacks 0x prefix: {value}"))?;
    u64::from_str_radix(digits, 16).with_context(|| format!("invalid address: {value}"))
}
