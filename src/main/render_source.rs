use anyhow::{Context, Result};
use puyo2_ps2_tool::source::{find_inventory_file, read_inventory_file};
use puyo2_ps2_tool::texture_source::TextureSourceInputs;

use super::{SourceArgs, inspect};

pub(super) struct LoadedTextureSource {
    source_bin_sha256: String,
    executable_iso_path: String,
    executable_sha256: String,
    executable: Vec<u8>,
    image_container_iso_path: String,
    image_container_sha256: String,
    image_container: Vec<u8>,
}

impl LoadedTextureSource {
    pub(super) fn inputs(&self) -> TextureSourceInputs<'_> {
        TextureSourceInputs {
            source_bin_sha256: self.source_bin_sha256.clone(),
            executable_iso_path: self.executable_iso_path.clone(),
            executable_sha256: self.executable_sha256.clone(),
            executable: &self.executable,
            image_container_iso_path: self.image_container_iso_path.clone(),
            image_container_sha256: self.image_container_sha256.clone(),
            image_container: &self.image_container,
        }
    }
}

pub(super) fn load_texture_source(
    source: &SourceArgs,
    executable_iso_path: String,
    image_container_iso_path: String,
) -> Result<LoadedTextureSource> {
    let inventory = inspect(source)?;
    let executable = read_inventory_file(&source.bin, &inventory, &executable_iso_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &executable_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("executable inventory entry has no SHA-256: {executable_iso_path}")
        })?;
    let image_container = read_inventory_file(&source.bin, &inventory, &image_container_iso_path)?;
    let image_container_sha256 = find_inventory_file(&inventory, &image_container_iso_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!("image inventory entry has no SHA-256: {image_container_iso_path}")
        })?;
    Ok(LoadedTextureSource {
        source_bin_sha256: inventory.source.bin.sha256,
        executable_iso_path,
        executable_sha256,
        executable,
        image_container_iso_path,
        image_container_sha256,
        image_container,
    })
}
