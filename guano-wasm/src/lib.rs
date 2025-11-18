use std::io::Cursor;

use guano_rs::{GuanoFile as RawGuanoFile, GuanoValue};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct GuanoFile {
    inner: RawGuanoFile,
}

#[wasm_bindgen]
impl GuanoFile {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<Self, String> {
        let cursor = Cursor::new(bytes);
        let inner = RawGuanoFile::new(cursor).map_err(|e| e.to_string())?;
        Ok(GuanoFile { inner })
    }

    #[wasm_bindgen]
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        if let Some(GuanoValue::String(s)) = self.inner.metadata().get(key) {
            Some(s.to_owned())
        } else {
            None
        }
    }

    #[wasm_bindgen]
    pub fn get_metadata_inside_namespace(&self, namespace: &str, key: &str) -> Option<String> {
        if self.inner.metadata().contains_key(namespace) {
            let kv = &self.inner.metadata()[namespace];
            if let GuanoValue::String(s) = &self.inner.metadata()[namespace][key] {
                Some(s.to_owned())
            } else {
                None
            }
        } else {
            None
        }
    }
    ///
    #[wasm_bindgen]
    pub fn metadata_keys(&self) -> Vec<String> {
        self.inner.metadata().keys().map(|s| s.to_owned()).collect()
    }

    #[wasm_bindgen]
    pub fn metadata_contains_key(&self, key: &str) -> bool {
        self.inner.metadata().contains_key(key)
    }

    #[wasm_bindgen]
    pub fn metadata_contains_key_inside_namespace(&self, namespace: &str, key: &str) -> bool {
        self.inner.metadata().contains_key(key)
    }
}
