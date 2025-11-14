use guano_rs::GuanoFile as RawGuanoFile;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct GuanoFile {
    _inner: RawGuanoFile,
}
