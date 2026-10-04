use anyhow::{Context, Result, ensure};

pub(crate) fn linked_texture_surface_file_stem(source_surface_id: &str) -> Result<String> {
    let suffix = source_surface_id
        .strip_prefix("linked-texture/")
        .context("linked-texture surface ID has an unexpected prefix")?;
    ensure!(
        suffix.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '/')
        }),
        "linked-texture surface ID cannot form a stable asset filename"
    );
    Ok(suffix.replace('/', "-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_texture_surface_id_maps_to_one_stable_filename() {
        assert_eq!(
            linked_texture_surface_file_stem("linked-texture/image-md/000249").unwrap(),
            "image-md-000249"
        );
    }

    #[test]
    fn linked_texture_surface_id_rejects_non_asset_components() {
        assert!(linked_texture_surface_file_stem("linked-texture/../IMAGE_MD").is_err());
        assert!(linked_texture_surface_file_stem("unlinked-texture/image-md/000249").is_err());
    }
}
