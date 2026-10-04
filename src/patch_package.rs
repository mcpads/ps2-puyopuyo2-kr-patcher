use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::source::{inspect_source, write_json_file};

#[derive(Deserialize)]
struct BuildIdentity {
    source_bin_sha256: String,
    output_bin_sha256: String,
    output_bin_byte_length: u64,
    included_components: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PatchIdentity {
    format: String,
    source_bin_sha256: String,
    target_bin_sha256: String,
    target_size: u64,
    patch_sha256: String,
    release_candidate_eligible: bool,
}

fn hash(path: &Path) -> Result<String> {
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let n = input.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        digest.update(&bytes[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn delta(tool: &Path, encode: bool, source: &Path, input: &Path, output: &Path) -> Result<()> {
    let mut command = Command::new(tool);
    command.env_remove("XDELTA");
    if encode {
        command.args(["-e", "-D", "-A", "-S", "none"]);
    } else {
        command.args(["-d", "-D", "-R"]);
    }
    let result = command
        .arg("-s")
        .arg(source)
        .arg(input)
        .arg(output)
        .output()
        .context("cannot execute xdelta3; install it or provide --xdelta")?;
    ensure!(
        result.status.success(),
        "xdelta3 failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

fn verify_target(path: &Path, expected_size: u64, expected_hash: &str) -> Result<()> {
    ensure!(
        fs::metadata(path)?.len() == expected_size,
        "patched BIN size mismatch"
    );
    ensure!(hash(path)? == expected_hash, "patched BIN SHA-256 mismatch");
    Ok(())
}

fn staging(destination: &Path) -> Result<tempfile::TempDir> {
    ensure!(
        !destination.exists(),
        "output directory already exists; refusing overwrite"
    );
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    Ok(tempfile::tempdir_in(parent)?)
}

fn publish(stage: tempfile::TempDir, destination: &Path) -> Result<()> {
    // Reserve the directory without replacing any existing user output.
    fs::create_dir(destination)?;
    let mut created = Vec::new();
    let result = (|| -> Result<()> {
        for entry in fs::read_dir(stage.path())? {
            let entry = entry?;
            let path = destination.join(entry.file_name());
            let mut input = File::open(entry.path())?;
            let mut output = File::options().write(true).create_new(true).open(&path)?;
            created.push(path);
            std::io::copy(&mut input, &mut output)?;
            output.flush()?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        for path in created {
            fs::remove_file(path).context("failed to remove incomplete output")?;
        }
        fs::remove_dir(destination).context("failed to remove incomplete output directory")?;
        return Err(error);
    }
    Ok(())
}

pub fn create(
    manifest: &Path,
    cue: &Path,
    source: &Path,
    target: &Path,
    report: &Path,
    destination: &Path,
    tool: &Path,
) -> Result<()> {
    let inventory = inspect_source(manifest, cue, source)?;
    let build: BuildIdentity = serde_json::from_slice(&fs::read(report)?)?;
    ensure!(
        build.included_components == 6 && build.source_bin_sha256 == inventory.source.bin.sha256,
        "build report does not describe the verified cumulative source"
    );
    verify_target(
        target,
        build.output_bin_byte_length,
        &build.output_bin_sha256,
    )?;
    let stage = staging(destination)?;
    let patch = stage.path().join("translation.xdelta");
    delta(tool, true, source, target, &patch)?;
    let roundtrip = tempfile::tempdir()?;
    let restored = roundtrip.path().join("restored.bin");
    delta(tool, false, source, &patch, &restored)?;
    verify_target(
        &restored,
        build.output_bin_byte_length,
        &build.output_bin_sha256,
    )?;
    let identity = PatchIdentity {
        format: "VCDIFF without secondary compression".into(),
        source_bin_sha256: build.source_bin_sha256,
        target_bin_sha256: build.output_bin_sha256,
        target_size: build.output_bin_byte_length,
        patch_sha256: hash(&patch)?,
        release_candidate_eligible: false,
    };
    fs::copy(manifest, stage.path().join("source.json"))?;
    write_json_file(&stage.path().join("patch.json"), &identity)?;
    fs::write(
        stage.path().join("README.txt"),
        "Local Korean patch candidate; not a release approval.\nRequires puyo2-ps2-tool and xdelta3.\nRun: puyo2-ps2-tool apply-patch --package PACKAGE --cue ORIGINAL.cue --bin ORIGINAL.bin --output-directory OUTPUT\nThe original BIN/CUE are verified, never modified. OUTPUT must not exist.\nOnly a fully decoded, size/hash-verified BIN is installed with its CUE.\nNo original media, emulator captures, or translation source is included.\n",
    )?;
    publish(stage, destination)
}

pub fn apply(
    package: &Path,
    cue: &Path,
    source: &Path,
    destination: &Path,
    tool: &Path,
) -> Result<()> {
    let identity: PatchIdentity = serde_json::from_slice(&fs::read(package.join("patch.json"))?)?;
    ensure!(
        identity.format == "VCDIFF without secondary compression",
        "unsupported patch format"
    );
    let inventory = inspect_source(&package.join("source.json"), cue, source)?;
    ensure!(
        inventory.source.bin.sha256 == identity.source_bin_sha256,
        "package source mismatch"
    );
    let patch = package.join("translation.xdelta");
    ensure!(
        hash(&patch)? == identity.patch_sha256,
        "patch SHA-256 mismatch"
    );
    let stage = staging(destination)?;
    let output = stage.path().join("puyo2-ko.bin");
    delta(tool, false, source, &patch, &output)?;
    verify_target(&output, identity.target_size, &identity.target_bin_sha256)?;
    fs::write(
        stage.path().join("puyo2-ko.cue"),
        "FILE \"puyo2-ko.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
    )?;
    publish(stage, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_wrong_target_and_existing_output() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("output.bin");
        fs::write(&file, b"correct length, wrong content").unwrap();
        assert!(verify_target(&file, 29, &"0".repeat(64)).is_err());
        assert!(staging(dir.path()).is_err());
        assert_eq!(fs::read(file).unwrap(), b"correct length, wrong content");
    }
}
