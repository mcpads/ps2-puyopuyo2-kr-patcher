use std::collections::BTreeMap;

use anyhow::Result;

use super::ExecutableStringReference;
use crate::executable_references::find_static_references;

pub(super) fn find_string_references(
    file: &object::File<'_>,
    target_addresses: &BTreeMap<u32, String>,
) -> Result<BTreeMap<String, Vec<ExecutableStringReference>>> {
    find_static_references(file, target_addresses)
}
