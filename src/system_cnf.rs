use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BootConfiguration {
    pub boot2: String,
    pub version: String,
    pub video_mode: String,
}

pub fn parse_system_cnf(bytes: &[u8]) -> Result<BootConfiguration> {
    ensure!(bytes.is_ascii(), "SYSTEM.CNF is not ASCII");
    let text = std::str::from_utf8(bytes)?;
    let mut values = BTreeMap::new();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("invalid SYSTEM.CNF line {}", line_index + 1))?;
        let key = key.trim().to_ascii_uppercase();
        let value = value.trim().to_string();
        ensure!(
            !key.is_empty(),
            "empty SYSTEM.CNF key at line {}",
            line_index + 1
        );
        ensure!(!value.is_empty(), "empty SYSTEM.CNF value for {key}");
        ensure!(
            values.insert(key.clone(), value).is_none(),
            "duplicate SYSTEM.CNF key {key}"
        );
    }
    Ok(BootConfiguration {
        boot2: take_required(&mut values, "BOOT2")?,
        version: take_required(&mut values, "VER")?,
        video_mode: take_required(&mut values, "VMODE")?,
    })
}

fn take_required(values: &mut BTreeMap<String, String>, key: &str) -> Result<String> {
    values
        .remove(key)
        .ok_or_else(|| anyhow::anyhow!("SYSTEM.CNF is missing {key}"))
}

#[cfg(test)]
#[path = "system_cnf/system_cnf_tests.rs"]
mod system_cnf_tests;
