use std::io::Cursor;

use guano_rs::{GuanoFile as RawGuanoFile, GuanoValue};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct GuanoFile {
    inner: RawGuanoFile,
}

#[wasm_bindgen]
impl GuanoFile {
    /// Creates a new GuanoFile for reading metadata.
    ///
    /// Reads the file as bytes.
    /// Returns a `Ok(Self)` if the metadata could be parsed successfully, otherwise an `Err(String)`
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<Self, String> {
        let cursor = Cursor::new(bytes);
        let inner = RawGuanoFile::new(cursor).map_err(|e| e.to_string())?;
        Ok(GuanoFile { inner })
    }

    /// Get the metadata for a particular key at the root namespace
    ///
    /// Returns `Some(String)` if the key exists, `None` if it does not.
    #[wasm_bindgen]
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        if let Some(GuanoValue::String(s)) = self.inner.metadata().get(key) {
            Some(s.to_owned())
        } else {
            None
        }
    }

    /// Get the metadata of a praticular namespace.
    ///
    /// Return `Some(String)` if the namespace and the key exists, otherwhise `None`.
    #[wasm_bindgen]
    pub fn get_metadata_inside_namespace(&self, namespace: &str, key: &str) -> Option<String> {
        if self.inner.metadata().contains_key(namespace) {
            match &self.inner.metadata()[namespace] {
                GuanoValue::String(v) => Some(v.to_owned()),
                GuanoValue::Object(hash_map) => match hash_map.get(key) {
                    Some(GuanoValue::String(v)) => Some(v.to_owned()),
                    _ => None,
                },
            }
        } else {
            None
        }
    }

    /// Returns a Vector containing all the keys of the GUANO metadata.
    ///
    /// For key-value pairs in the root namespace, the key is simply returned
    /// For namespaced key-value pairs, the keys are returned as `"<namespace>|<key>"`
    #[wasm_bindgen]
    pub fn metadata_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();

        for (key, value) in self.inner.metadata().iter() {
            match value {
                GuanoValue::String(_) => {
                    // Root namespace key-value pair
                    keys.push(key.to_owned());
                }
                GuanoValue::Object(namespace) => {
                    // Namespace - add all keys in "<namespace>|<key>" format
                    for nested_key in namespace.keys() {
                        keys.push(format!("{}|{}", key, nested_key));
                    }
                }
            }
        }

        keys
    }

    /// Test if the metadata contains a certain key.
    ///
    /// Returns `true`, if the key is present at the root namespace
    #[wasm_bindgen]
    pub fn metadata_contains(&self, key: &str) -> bool {
        self.inner.metadata().contains_key(key)
    }

    /// Test if the metadata contains a certain key under a certain namespace.
    ///
    /// Return `true`, if the key is present at the namespace
    #[wasm_bindgen]
    pub fn metadata_namespace_contains(&self, namespace: &str, key: &str) -> bool {
        if self.inner.metadata().contains_key(namespace) {
            match &self.inner.metadata()[namespace] {
                GuanoValue::String(_) => false,
                GuanoValue::Object(hash_map) => hash_map.contains_key(key),
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod test {

    use core::panic;

    use wasm_bindgen_test::wasm_bindgen_test;

    const TEST_METADATA: &str = "Model: Song Meter Mini
Make: Wildlife Acoustics, Inc.
Original Filename: 2MA04827_20250227_073902.wav
Timestamp: 2025-02-27T07:39:02+01:00
Samplerate: 24000
GUANO|Version: 1.0
WA|Song Meter|Prefix: 2MA04827
WA|Song Meter|Audio settings: [{\"rate\":24000,\"gain\":18}]
Firmware Version: 4.6
Loc Position: 46.765390 8.738140
Serial: 2MA04827
Length: 3597.99
Temperature Int: -1.25";
    use super::*;

    fn get_file_as_bytes_vec() -> Vec<u8> {
        let mut file = format!("RIFF0000WAVEdata0000AAAAguan0000{TEST_METADATA}").into_bytes();
        let file_size = (file.len() as u32).to_le_bytes();
        file[4..8].copy_from_slice(&file_size);
        let data_size = 4_u32.to_le_bytes();
        file[16..20].copy_from_slice(&data_size);
        let guano_size = TEST_METADATA.len().to_le_bytes();
        file[28..32].copy_from_slice(&guano_size);
        file
    }

    #[wasm_bindgen_test]
    fn test_open() {
        let file = &get_file_as_bytes_vec()[..];
        match GuanoFile::new(file) {
            Ok(_) => (),
            Err(e) => panic!("Expected Ok, got Err('{}')", e),
        }
    }

    #[wasm_bindgen_test]
    fn test_metadata_contains_existing_root_key() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");

        assert!(gf.metadata_contains("Make"));
        assert!(gf.metadata_contains("Timestamp"));
        assert!(gf.metadata_contains("Serial"));
    }

    #[wasm_bindgen_test]
    fn test_metadata_contains_nonexistent_key() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");

        assert!(!gf.metadata_contains("NonExistent"));
        assert!(!gf.metadata_contains("FakeKey"));
    }

    #[wasm_bindgen_test]
    fn test_metadata_contains_namespace_key() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");

        // Namespace keys should exist at root level
        assert!(gf.metadata_contains("GUANO"));
        assert!(gf.metadata_contains("WA"));
    }

    #[wasm_bindgen_test]
    fn test_metadata_namespace_contains_existing_key() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");
        let out: Vec<&String> = gf.inner.metadata().keys().collect();
        assert!(gf.metadata_namespace_contains("GUANO", "Version"));
        assert!(gf.metadata_namespace_contains("WA", "Song Meter|Prefix"));
        assert!(gf.metadata_namespace_contains("WA", "Song Meter|Audio settings"));
    }

    #[wasm_bindgen_test]
    fn test_metadata_namespace_contains_nonexistent_key() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");

        assert!(!gf.metadata_namespace_contains("GUANO", "NonExistent"));
        assert!(!gf.metadata_namespace_contains("WA", "FakeKey"));
    }

    #[wasm_bindgen_test]
    fn test_metadata_namespace_contains_nonexistent_namespace() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");

        assert!(!gf.metadata_namespace_contains("NonExistentNamespace", "Version"));
        assert!(!gf.metadata_namespace_contains("FakeNamespace", "Key"));
    }

    #[wasm_bindgen_test]
    fn test_get_metadata_returns_some() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");

        assert_eq!(
            gf.get_metadata("Model"),
            Some("Song Meter Mini".to_string())
        );
        assert_eq!(gf.get_metadata("Serial"), Some("2MA04827".to_string()));
        assert_eq!(
            gf.get_metadata("Timestamp"),
            Some("2025-02-27T07:39:02+01:00".to_string())
        );
    }

    #[wasm_bindgen_test]
    fn test_get_metadata_returns_none() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");

        assert_eq!(gf.get_metadata("NonExistent"), None);
        assert_eq!(gf.get_metadata("FakeKey"), None);
        // Namespace keys should return None when accessed as root keys
        assert_eq!(gf.get_metadata("GUANO"), None);
    }

    #[wasm_bindgen_test]
    fn test_get_metadata_inside_namespace_returns_some() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");

        assert_eq!(
            gf.get_metadata_inside_namespace("GUANO", "Version"),
            Some("1.0".to_string())
        );
        assert_eq!(
            gf.get_metadata_inside_namespace("WA", "Song Meter|Prefix"),
            Some("2MA04827".to_string())
        );
        assert_eq!(
            gf.get_metadata_inside_namespace("WA", "Song Meter|Audio settings"),
            Some("[{\"rate\":24000,\"gain\":18}]".to_string())
        );
    }

    #[wasm_bindgen_test]
    fn test_get_metadata_inside_namespace_returns_none() {
        let file = &get_file_as_bytes_vec()[..];
        let gf = GuanoFile::new(file).expect("Failed to create GuanoFile");

        // Non-existent key in existing namespace
        assert_eq!(
            gf.get_metadata_inside_namespace("GUANO", "NonExistent"),
            None
        );
        assert_eq!(gf.get_metadata_inside_namespace("WA", "FakeKey"), None);

        // Non-existent namespace
        assert_eq!(
            gf.get_metadata_inside_namespace("NonExistentNamespace", "Version"),
            None
        );
    }
}
