use std::path::{Path, PathBuf};

use anyhow::{Result, bail, ensure};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CueSheet {
    pub data_file: String,
    pub track_number: u8,
    pub track_mode: String,
    pub index_number: u8,
    pub index_start: String,
}

impl CueSheet {
    pub fn parse(text: &str) -> Result<Self> {
        ensure!(text.is_ascii(), "CUE must be ASCII");
        let mut data_file = None;
        let mut track_number = None;
        let mut track_mode = None;
        let mut index_number = None;
        let mut index_start = None;

        for (line_index, raw_line) in text.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            let keyword = line
                .split_ascii_whitespace()
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            match keyword.as_str() {
                "FILE" => {
                    ensure!(data_file.is_none(), "CUE must contain exactly one FILE");
                    ensure!(
                        track_number.is_none() && index_number.is_none(),
                        "FILE appears after TRACK or INDEX at line {line_number}"
                    );
                    data_file = Some(parse_file_line(line, line_number)?);
                }
                "TRACK" => {
                    ensure!(
                        data_file.is_some(),
                        "TRACK appears before FILE at line {line_number}"
                    );
                    ensure!(track_number.is_none(), "CUE must contain exactly one TRACK");
                    let fields: Vec<_> = line.split_ascii_whitespace().collect();
                    ensure!(fields.len() == 3, "invalid TRACK at line {line_number}");
                    track_number = Some(fields[1].parse::<u8>()?);
                    track_mode = Some(fields[2].to_ascii_uppercase());
                }
                "INDEX" => {
                    ensure!(
                        track_number.is_some(),
                        "INDEX appears before TRACK at line {line_number}"
                    );
                    ensure!(index_number.is_none(), "CUE must contain exactly one INDEX");
                    let fields: Vec<_> = line.split_ascii_whitespace().collect();
                    ensure!(fields.len() == 3, "invalid INDEX at line {line_number}");
                    index_number = Some(fields[1].parse::<u8>()?);
                    validate_msf(fields[2])?;
                    index_start = Some(fields[2].to_string());
                }
                "FLAGS" => {
                    ensure!(
                        track_number.is_some(),
                        "FLAGS appears before TRACK at line {line_number}"
                    );
                    let fields: Vec<_> = line.split_ascii_whitespace().collect();
                    ensure!(
                        fields.len() == 2 && fields[1].eq_ignore_ascii_case("DCP"),
                        "only FLAGS DCP is supported at line {line_number}"
                    );
                }
                "REM" | "TITLE" | "PERFORMER" | "CATALOG" => {}
                _ => bail!("unsupported CUE line {line_number}: {raw_line:?}"),
            }
        }

        Ok(Self {
            data_file: data_file.ok_or_else(|| anyhow::anyhow!("CUE does not contain FILE"))?,
            track_number: track_number
                .ok_or_else(|| anyhow::anyhow!("CUE does not contain TRACK"))?,
            track_mode: track_mode
                .ok_or_else(|| anyhow::anyhow!("CUE does not contain track mode"))?,
            index_number: index_number
                .ok_or_else(|| anyhow::anyhow!("CUE does not contain INDEX"))?,
            index_start: index_start
                .ok_or_else(|| anyhow::anyhow!("CUE does not contain index start"))?,
        })
    }

    pub fn referenced_path(&self, cue_path: &Path) -> PathBuf {
        cue_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&self.data_file)
    }
}

fn parse_file_line(line: &str, line_number: usize) -> Result<String> {
    let first_quote = line
        .find('"')
        .ok_or_else(|| anyhow::anyhow!("FILE must use quotes at line {line_number}"))?;
    ensure!(
        line[..first_quote].trim().eq_ignore_ascii_case("FILE"),
        "invalid FILE prefix at line {line_number}"
    );
    let remaining = &line[first_quote + 1..];
    let second_quote = remaining
        .find('"')
        .ok_or_else(|| anyhow::anyhow!("unterminated FILE quote at line {line_number}"))?;
    let data_file = &remaining[..second_quote];
    ensure!(
        !data_file.is_empty(),
        "empty FILE name at line {line_number}"
    );
    ensure!(
        remaining[second_quote + 1..]
            .trim()
            .eq_ignore_ascii_case("BINARY"),
        "FILE must use BINARY at line {line_number}"
    );
    Ok(data_file.to_string())
}

fn validate_msf(value: &str) -> Result<()> {
    let fields: Vec<_> = value.split(':').collect();
    ensure!(fields.len() == 3, "invalid CUE timestamp {value}");
    let minute = fields[0].parse::<u32>()?;
    let second = fields[1].parse::<u32>()?;
    let frame = fields[2].parse::<u32>()?;
    ensure!(second < 60 && frame < 75, "invalid CUE timestamp {value}");
    minute
        .checked_mul(60)
        .and_then(|total| total.checked_add(second))
        .and_then(|total| total.checked_mul(75))
        .and_then(|total| total.checked_add(frame))
        .ok_or_else(|| anyhow::anyhow!("CUE timestamp overflow {value}"))?;
    Ok(())
}

#[cfg(test)]
#[path = "cue/cue_tests.rs"]
mod cue_tests;
