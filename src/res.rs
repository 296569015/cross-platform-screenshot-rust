use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
pub struct Asset;

pub fn get(path: &str) -> Option<Vec<u8>> {
    Asset::get(path).map(|asset| asset.data.into_owned())
}
