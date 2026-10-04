pub struct TextureSourceInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
    pub image_container_iso_path: String,
    pub image_container_sha256: String,
    pub image_container: &'a [u8],
}
