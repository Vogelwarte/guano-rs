use guano_rs::GuanoFile as RawGuanoFile;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct GuanoFile {
    inner: RawGuanoFile<std::io::Cursor<&'static [u8]>>,
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet(name: &str) {
    alert(&format!("Hello, {name}!"));
}
