///
/// This is an independant implementation for reading and writing GUANO metadata.
/// The authors of this package are not associated with the authors of the reference implementation.
///
///
#[cfg(feature = "serde")]
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, Visitor},
    ser::SerializeMap,
};
#[cfg(feature = "serde")]
use std::fmt;
use std::{
    collections::HashMap,
    io::{self, Read, Seek},
    ops::Index,
};
use thiserror::Error;

/// Represents a WAV file containing GUANO metadata.
///
/// This struct provides read-only access to GUANO (Grand Unified Acoustic Notation Ontology)
/// metadata stored in WAV files. GUANO metadata is commonly used for bat acoustic recordings
/// and is stored in the `guan` RIFF chunk within the WAV file.
///
/// # Examples
///
/// ```rust
/// use std::fs::File;
/// use guano::GuanoFile;
///
/// let file = File::open("testdata/recording.wav").unwrap();
/// let guano = GuanoFile::new(file).unwrap();
/// let metadata = guano.metadata();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuanoFile {
    map: HashMap<String, GuanoValue>,
}

/// Represents a value in the GUANO metadata structure.
///
/// GUANO metadata can contain either simple string values or nested objects
/// (for namespaced metadata fields). Use pattern matching or the `Index` trait
/// to access values.
///
/// # Examples
///
/// ```rust
/// use guano::{GuanoFile, GuanoValue};
/// use std::fs::File;
///
/// let file = File::open("testdata/recording.wav").unwrap();
/// let guano = GuanoFile::new(file).unwrap();
///
/// // Access a string value
/// if let Some(GuanoValue::String(timestamp)) = guano.metadata().get("Timestamp") {
///     println!("Timestamp: {}", timestamp);
/// }
///
/// // Access a nested object
/// if let Some(GuanoValue::Object(guano_ns)) = guano.metadata().get("GUANO") {
///     if let Some(GuanoValue::String(version)) = guano_ns.get("Version") {
///         println!("GUANO Version: {}", version);
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuanoValue {
    /// A simple string value
    String(String),
    /// A nested object containing namespaced metadata fields
    Object(HashMap<String, GuanoValue>),
}

/// Trims leading and trailing whitespace as the GUANO specification defines it.
///
/// GUANO's notion of whitespace is wider than Rust's `char::is_whitespace`: the spec
/// states that whitespace "should include the non-printing ASCII bytes including null,
/// CR, LF, space, tab, etc.". Because `str::trim` does not strip NUL, recorders that pad
/// the `guan` chunk with NUL bytes (as the Wildlife Acoustics Song Meter Mini does) would
/// otherwise produce a trailing line that fails to parse.
fn trim_guano(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c.is_control())
}

/// Returns the keys of `map` sorted alphabetically.
///
/// `HashMap` iteration order is unspecified and varies between runs, so serializing straight
/// from the map would emit the same file differently every time and make output impossible to
/// diff or checksum.
#[cfg(feature = "serde")]
fn sorted_keys(map: &HashMap<String, GuanoValue>) -> Vec<&String> {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort_unstable();
    keys
}

/// Returns the root keys of `map` in GUANO output order: the `GUANO` namespace first, then
/// the rest alphabetically.
///
/// The spec requires `GUANO|Version` to be the very first field, so its namespace leads.
#[cfg(feature = "serde")]
fn ordered_keys(map: &HashMap<String, GuanoValue>) -> Vec<&String> {
    let mut keys = sorted_keys(map);
    // `sort_by_key` is stable, so the alphabetical order established above survives for
    // everything that is not the GUANO namespace.
    keys.sort_by_key(|k| k.as_str() != "GUANO");
    keys
}

impl Index<&str> for GuanoValue {
    type Output = GuanoValue;

    /// Provides index access to nested `GuanoValue::Object` values.
    ///
    /// # Panics
    ///
    /// This implementation will panic in two cases:
    /// 1. If called on a `GuanoValue::String` variant (strings are not indexable)
    /// 2. If the key does not exist in a `GuanoValue::Object` variant
    ///
    /// For safer access, use pattern matching or the `get()` method on the underlying `HashMap`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use guano::{GuanoFile, GuanoValue};
    /// use std::fs::File;
    ///
    /// let file = File::open("testdata/recording.wav").unwrap();
    /// let guano = GuanoFile::new(file).unwrap();
    ///
    /// // Safe: Check type first
    /// if let GuanoValue::Object(_) = &guano.metadata()["GUANO"] {
    ///     let version = &guano.metadata()["GUANO"]["Version"];
    /// }
    ///
    /// // Unsafe: Will panic if "Timestamp" is a String or doesn't exist
    /// // let version = &guano.metadata()["Timestamp"]["Timezone"];
    /// ```
    fn index(&self, index: &str) -> &Self::Output {
        match self {
            GuanoValue::String(_) => panic!("Cannot index into GuanoValue::String"),
            GuanoValue::Object(hash_map) => &hash_map[index],
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for GuanoValue {
    /// Serializes a value *transparently*: a [`GuanoValue::String`] becomes a plain string and
    /// a [`GuanoValue::Object`] becomes a map.
    ///
    /// This is deliberately not a derived implementation, which would emit the externally
    /// tagged form `{"String": "1.0"}` and leak the Rust enum into the output.
    ///
    /// Keys inside an object are emitted alphabetically, so the same metadata always produces
    /// the same bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use guano::GuanoValue;
    ///
    /// let version = GuanoValue::String("1.0".to_owned());
    /// assert_eq!(serde_json::to_string(&version)?, r#""1.0""#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            GuanoValue::String(s) => serializer.serialize_str(s),
            GuanoValue::Object(hash_map) => {
                let mut map = serializer.serialize_map(Some(hash_map.len()))?;
                for key in sorted_keys(hash_map) {
                    map.serialize_entry(key, &hash_map[key])?;
                }
                map.end()
            }
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for GuanoValue {
    /// Deserializes a string into [`GuanoValue::String`] and a map into
    /// [`GuanoValue::Object`], mirroring [`GuanoValue`]'s [`Serialize`] implementation.
    ///
    /// Namespace entries are read as strings, which rejects both non-string scalars
    /// (`{"Samplerate": 22050}`) and nesting deeper than one level. That restriction is not
    /// arbitrary: GUANO is a flat `namespace|key: value` text format, so neither shape could
    /// ever be written back into a `guan` chunk.
    ///
    /// Note that this uses `deserialize_any`, so it requires a self-describing format such as
    /// JSON or YAML. Formats like bincode, which do not record types on the wire, will fail.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use guano::GuanoValue;
    ///
    /// let value: GuanoValue = serde_json::from_str(r#""Song Meter Mini""#)?;
    /// assert_eq!(value, GuanoValue::String("Song Meter Mini".to_owned()));
    ///
    /// // A number is not a GUANO value: every field is text.
    /// assert!(serde_json::from_str::<GuanoValue>("22050").is_err());
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GuanoValueVisitor;

        impl<'de> Visitor<'de> for GuanoValueVisitor {
            type Value = GuanoValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "a GUANO string value, or a namespace mapping field names to string values",
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(GuanoValue::String(v.to_owned()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(GuanoValue::String(v))
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                // Not `with_capacity(size_hint)`: the hint comes from the input and a hostile
                // one would let a tiny document allocate an enormous map.
                let mut hash_map = HashMap::new();
                while let Some((key, value)) = access.next_entry::<String, String>()? {
                    hash_map.insert(key, GuanoValue::String(value));
                }
                Ok(GuanoValue::Object(hash_map))
            }
        }

        deserializer.deserialize_any(GuanoValueVisitor)
    }
}

#[cfg(feature = "serde")]
impl Serialize for GuanoFile {
    /// Serializes the metadata map itself, so a file becomes a flat mapping of field names to
    /// values rather than a wrapper object around it.
    ///
    /// The `GUANO` namespace is emitted first, because the specification requires
    /// `GUANO|Version` to be the first field; the remaining root keys follow alphabetically.
    /// The order is fixed rather than inherited from the `HashMap`, so serializing the same
    /// file twice always yields identical output.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::fs::File;
    /// use guano::GuanoFile;
    ///
    /// let guano = GuanoFile::new(File::open("testdata/recording.wav")?)?;
    /// let json = serde_json::to_string_pretty(&guano)?;
    ///
    /// // The GUANO namespace leads, as the specification requires.
    /// assert!(json.starts_with("{\n  \"GUANO\": {"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.map.len()))?;
        for key in ordered_keys(&self.map) {
            map.serialize_entry(key, &self.map[key])?;
        }
        map.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for GuanoFile {
    /// Rebuilds a `GuanoFile` from a mapping of field names to values, as produced by
    /// [`GuanoFile`]'s [`Serialize`] implementation.
    ///
    /// The resulting value carries metadata only; it is not attached to a WAV file, and this
    /// crate has no way to write it back into one.
    ///
    /// Values are deserialized through [`GuanoValue`], so the same one-level-of-nesting and
    /// strings-only restrictions apply.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use guano::{GuanoFile, GuanoValue};
    ///
    /// let json = r#"{"GUANO": {"Version": "1.0"}, "Make": "Wildlife Acoustics, Inc."}"#;
    /// let guano: GuanoFile = serde_json::from_str(json)?;
    ///
    /// assert_eq!(guano.metadata()["GUANO"]["Version"], GuanoValue::String("1.0".to_owned()));
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(GuanoFile {
            map: HashMap::deserialize(deserializer)?,
        })
    }
}

impl GuanoFile {
    /// Creates a new `GuanoFile` by parsing GUANO metadata from a WAV file.
    ///
    /// This function reads the WAV file structure, locates the `guan` RIFF chunk,
    /// and parses the GUANO metadata into a structured format.
    ///
    /// # Arguments
    ///
    /// * `file` - An open file handle to a WAV file containing GUANO metadata
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the parsed `GuanoFile` or a `GuanoError` if:
    /// - The file is not a valid WAV file
    /// - The file does not contain a `guan` chunk
    /// - An I/O error occurs while reading the file
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::fs::File;
    /// use guano::GuanoFile;
    ///
    /// let file = File::open("testdata/recording.wav")?;
    /// let guano = GuanoFile::new(file)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new<T: Read + Seek>(reader: T) -> Result<Self, GuanoError> {
        let mut gf = GuanoFile {
            map: HashMap::new(),
        };
        gf.load(reader)?;
        Ok(gf)
    }

    /// Returns a reference to the parsed GUANO metadata.
    ///
    /// The metadata is returned as a `HashMap` where keys are field names and values
    /// are `GuanoValue` enums. Root-level fields are stored directly in the map,
    /// while namespaced fields (e.g., `GUANO|Version`) are stored as nested objects
    /// under their namespace key.
    ///
    /// # Returns
    ///
    /// A reference to the metadata `HashMap<String, GuanoValue>`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::fs::File;
    /// use guano::{GuanoFile, GuanoValue};
    ///
    /// let file = File::open("testdata/recording.wav")?;
    /// let guano = GuanoFile::new(file)?;
    /// let metadata = guano.metadata();
    ///
    /// // Access root-level field
    /// if let Some(GuanoValue::String(ts)) = metadata.get("Timestamp") {
    ///     println!("Recording timestamp: {}", ts);
    /// }
    ///
    /// // Access namespaced field using index syntax
    /// let version = &metadata["GUANO"]["Version"];
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn metadata(&self) -> &HashMap<String, GuanoValue> {
        &self.map
    }

    fn load<T: Read + Seek>(&mut self, mut reader: T) -> Result<(), GuanoError> {
        let reader = &mut reader;
        // check the file size. A valid RIFF header must be at least 8 bytes in size
        let file_len = reader.seek(io::SeekFrom::End(0))?;
        if file_len < 8 {
            return Err(GuanoError::FileHeaderError(
                "File too small to contain RIFF \"WAVE\" header".to_owned(),
            ));
        }
        // check that the file contains the "WAVE" RIFF chunk at 0x08
        reader.seek(io::SeekFrom::Start(0x08))?;
        let mut header = [0u8; 4];
        reader.read_exact(&mut header)?;
        if header != c"WAVE".to_bytes() {
            // Escape the bytes we found: a zeroed-out file would otherwise report
            // invisible NULs.
            return Err(GuanoError::FileHeaderError(format!(
                "Expected RIFF chunk \"WAVE\", but found \"{}\"",
                String::from_utf8_lossy(&header).escape_debug()
            )));
        }

        let mut chunkid_buf = [0u8; 4];
        let mut chunksz_buf = [0u8; 4];
        let mut chunksz;
        let mut found_guano = false;
        reader.seek(io::SeekFrom::Start(0x0C))?;
        loop {
            // read chunk id big endian
            if reader.read_exact(&mut chunkid_buf).is_err() {
                // Reached end of file
                break;
            }
            // read chunk size as little endian
            reader.read_exact(&mut chunksz_buf)?;
            chunksz = u32::from_le_bytes(chunksz_buf) as usize;

            // A chunk that runs past the end of the file means the file is truncated.
            // Without this check the walk would seek past EOF and the next read would
            // simply end the loop, misreporting a damaged file as "no GUANO metadata".
            let body_start = reader.stream_position()?;
            let available = file_len - body_start;
            if chunksz as u64 > available {
                return Err(GuanoError::TruncatedFile {
                    chunk: String::from_utf8_lossy(&chunkid_buf).into_owned(),
                    declared_size: chunksz as u64,
                    available,
                });
            }

            // if "guan" is present, extract the metadata
            if chunkid_buf == c"guan".to_bytes() {
                let mut metadata_buf = vec![0; chunksz];
                reader.read_exact(&mut metadata_buf[0..chunksz])?;
                self.parse(&metadata_buf)?;
                found_guano = true;
                break;
            } else {
                reader.seek_relative(chunksz.try_into().unwrap())?;
            }
            if chunksz % 2 != 0 {
                reader.seek_relative(1)?;
            }
        }

        if !found_guano {
            return Err(GuanoError::NoGuanoMetadata);
        }

        Ok(())
    }

    fn parse(&mut self, raw: &[u8]) -> Result<(), GuanoError> {
        // check if the str can be interpreted directly
        let str = str::from_utf8(raw)
            .map_err(|e| GuanoError::MalformedMetadata(format!("Invalid UTF-8: {}", e)))?;

        for (idx, raw_line) in str.lines().enumerate() {
            let line = trim_guano(raw_line);
            if line.is_empty() {
                continue; // Skip empty lines
            }

            let kv: Vec<&str> = line.splitn(2, ':').collect();
            if kv.len() != 2 {
                return Err(GuanoError::MalformedMetadata(format!(
                    "Expected key:value format on line {}, found: '{}'",
                    idx + 1,
                    line.escape_debug()
                )));
            }

            let full_key = trim_guano(kv[0]);
            let val = trim_guano(kv[1]).to_owned();

            // check to see if the key has a namespace
            if full_key.contains('|') {
                let ns: Vec<&str> = full_key.splitn(2, '|').collect();
                if ns.len() != 2 {
                    return Err(GuanoError::MalformedMetadata(format!(
                        "Malformed namespace in key: '{}'",
                        full_key
                    )));
                }

                if !self.map.contains_key(ns[0]) {
                    self.map
                        .insert(ns[0].to_owned(), GuanoValue::Object(HashMap::new()));
                }
                if let Some(GuanoValue::Object(m)) = self.map.get_mut(ns[0]) {
                    m.insert(ns[1].to_owned(), GuanoValue::String(val));
                }
            } else {
                self.map
                    .insert(full_key.to_owned(), GuanoValue::String(val));
            }
        }

        Ok(())
    }
}

/// Errors that can occur when reading GUANO metadata from WAV files.
#[derive(Error, Debug)]
pub enum GuanoError {
    /// An I/O error occurred while reading the file.
    ///
    /// This variant wraps standard I/O errors that may occur during file operations,
    /// such as permission issues, disk errors, or the file not existing.
    #[error("File IO Error")]
    FileIOError(#[from] io::Error),

    /// The file does not contain a valid RIFF WAVE header or structure.
    ///
    /// This error occurs when:
    /// - The file is too small to contain a valid RIFF header
    /// - The WAVE format identifier is missing or incorrect
    /// - The file structure does not conform to the RIFF WAVE specification
    #[error("RIFF \"WAVE\" header error: {0}")]
    FileHeaderError(String),

    /// The WAV file does not contain a GUANO metadata chunk.
    ///
    /// This error occurs when the file is a valid WAV file but does not contain
    /// the `guan` RIFF chunk that stores GUANO metadata.
    #[error("No GUANO metadata chunk found in file")]
    NoGuanoMetadata,

    /// A RIFF chunk declares more bytes than the file actually contains.
    ///
    /// The file is damaged or was truncated, for example when a recorder lost power
    /// mid-write. This is deliberately distinct from [`GuanoError::NoGuanoMetadata`]:
    /// "this file is damaged" and "this recorder wrote no GUANO" call for very
    /// different responses when triaging field recordings.
    #[error(
        "Truncated file: chunk \"{chunk}\" declares {declared_size} bytes, but only {available} remain"
    )]
    TruncatedFile {
        /// The four character id of the offending chunk.
        chunk: String,
        /// The chunk length declared in the chunk header.
        declared_size: u64,
        /// The number of bytes actually remaining in the file.
        available: u64,
    },

    /// The GUANO metadata is malformed and cannot be parsed.
    ///
    /// This error occurs when:
    /// - Metadata lines do not contain the expected key:value format
    /// - Namespaced keys are malformed
    /// - The metadata contains invalid UTF-8 sequences
    #[error("Malformed GUANO metadata: {0}")]
    MalformedMetadata(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn open_file() -> Result<(), GuanoError> {
        let f = File::open("testdata/recording.wav")?;
        GuanoFile::new(f)?;
        Ok(())
    }

    #[test]
    fn has_guano_version() -> Result<(), GuanoError> {
        let f = File::open("testdata/recording.wav")?;
        let gf = GuanoFile::new(f)?;
        // test some required params
        assert!(gf.metadata().get("GUANO").is_some());
        // test root level namespace param
        assert!(matches!(gf.metadata()["Timestamp"], GuanoValue::String(_)));
        // test hierarchichal param
        assert!(matches!(
            gf.metadata()["GUANO"]["Version"],
            GuanoValue::String(_)
        ));
        Ok(())
    }
    #[test]
    fn has_no_guano_metadata() -> Result<(), io::Error> {
        let f = File::open("testdata/no_meta.wav")?;
        let expected_err = GuanoFile::new(f).unwrap_err();
        assert!(matches!(expected_err, GuanoError::NoGuanoMetadata));
        Ok(())
    }

    #[test]
    fn too_small() -> Result<(), io::Error> {
        let f = File::open("testdata/smol.wav")?;
        let expected_err = GuanoFile::new(f).unwrap_err();
        assert!(matches!(expected_err, GuanoError::FileHeaderError(_)));
        let GuanoError::FileHeaderError(msg) = expected_err else {
            unreachable!("Panic happend before, if the expected_err is not FileHeaderError");
        };
        assert!(msg.contains("too small"));
        Ok(())
    }

    /// `testdata/weird.wav` is deliberately weird but fully spec-legal, so every
    /// assertion below must hold. See `testdata/generate_weird.py` for its contents.
    mod weird {
        use super::*;

        fn weird() -> GuanoFile {
            let f = File::open("testdata/weird.wav").expect("testdata/weird.wav is missing");
            GuanoFile::new(f).expect("weird.wav is spec-legal and must parse")
        }

        fn root(gf: &GuanoFile, key: &str) -> String {
            match gf.metadata().get(key) {
                Some(GuanoValue::String(s)) => s.to_owned(),
                other => panic!("expected a string for '{key}', got {other:?}"),
            }
        }

        fn nested(gf: &GuanoFile, ns: &str, key: &str) -> String {
            match gf.metadata().get(ns) {
                Some(GuanoValue::Object(m)) => match m.get(key) {
                    Some(GuanoValue::String(s)) => s.to_owned(),
                    other => panic!("expected a string for '{ns}|{key}', got {other:?}"),
                },
                other => panic!("expected an object for namespace '{ns}', got {other:?}"),
            }
        }

        /// The bug this fixture exists for: a NUL padding byte must not leak into a value.
        #[test]
        fn nul_padding_is_trimmed() {
            assert_eq!(root(&weird(), "Trailing NUL Value"), "22050");
        }

        /// An odd sized chunk before `guan` means a broken RIFF pad byte skip
        /// desynchronizes the walk and `guan` is never found.
        #[test]
        fn finds_guan_after_odd_sized_chunk() {
            assert!(weird().metadata().contains_key("Model"));
        }

        #[test]
        fn trims_padded_key_and_value() {
            assert_eq!(root(&weird(), "Make"), "Wildlife Acoustics, Inc.");
        }

        #[test]
        fn keeps_colons_inside_value() {
            assert_eq!(root(&weird(), "Timestamp"), "2026-01-15 08:12:02+01:00");
        }

        #[test]
        fn strips_carriage_return() {
            assert_eq!(root(&weird(), "Original Filename"), "weird.wav");
        }

        #[test]
        fn keeps_multibyte_utf8() {
            assert_eq!(root(&weird(), "Note"), "Waldkauz – Käuzchen, 5 °C");
        }

        #[test]
        fn keeps_empty_value() {
            assert_eq!(root(&weird(), "Empty Value"), "");
        }

        /// Escape sequences are not unescaped yet, so `\n` must survive as two
        /// characters rather than becoming a newline.
        #[test]
        fn does_not_unescape_values() {
            assert_eq!(root(&weird(), "Escaped"), "line one\\nline two");
        }

        #[test]
        fn splits_namespace_on_first_pipe_only() {
            let gf = weird();
            assert_eq!(nested(&gf, "GUANO", "Version"), "1.0");
            assert_eq!(nested(&gf, "WA", "Song Meter|Prefix"), "TANIAULMET");
            assert_eq!(
                nested(&gf, "WA", "Song Meter|Audio settings"),
                r#"[{"rate":22050,"gain":18}]"#
            );
        }

        /// The blank line and the whitespace-plus-NUL line must not become entries.
        #[test]
        fn skips_blank_and_whitespace_only_lines() {
            let gf = weird();
            let root_keys = gf.metadata().len();
            let wa_keys = match &gf.metadata()["WA"] {
                GuanoValue::Object(m) => m.len(),
                _ => panic!("WA should be a namespace"),
            };
            // 12 plain keys + the GUANO and WA namespaces
            assert_eq!(root_keys, 14, "unexpected root keys: {:?}", gf.metadata());
            assert_eq!(wa_keys, 2);
        }
    }

    /// Corrupt metadata must keep failing loudly. A silently half-parsed file is
    /// worse than a rejected one when triaging recorder output.
    mod still_rejects {
        use super::*;
        use std::io::Cursor;

        /// Builds an in-memory RIFF/WAVE file wrapping `guan_payload`.
        fn wav_with_guano(guan_payload: &[u8]) -> Vec<u8> {
            let mut body = Vec::new();
            body.extend_from_slice(b"data");
            body.extend_from_slice(&4u32.to_le_bytes());
            body.extend_from_slice(&[0u8; 4]);
            body.extend_from_slice(b"guan");
            body.extend_from_slice(&(guan_payload.len() as u32).to_le_bytes());
            body.extend_from_slice(guan_payload);

            let mut file = Vec::from(*b"RIFF");
            file.extend_from_slice(&((4 + body.len()) as u32).to_le_bytes());
            file.extend_from_slice(b"WAVE");
            file.extend_from_slice(&body);
            file
        }

        #[test]
        fn line_without_a_colon() {
            let wav = wav_with_guano(b"GUANO|Version:1.0\ngarbage line without a colon\n");
            let err = GuanoFile::new(Cursor::new(wav)).unwrap_err();
            let GuanoError::MalformedMetadata(msg) = err else {
                panic!("expected MalformedMetadata, got {err:?}");
            };
            assert!(
                msg.contains("line 2"),
                "error should locate the line: {msg}"
            );
        }

        /// Non printing bytes must be escaped in the message, otherwise the operator
        /// sees an empty pair of quotes and cannot tell what went wrong.
        #[test]
        fn error_message_escapes_non_printing_bytes() {
            let wav = wav_with_guano(b"GUANO|Version:1.0\nno colon \x07 here\n");
            let err = GuanoFile::new(Cursor::new(wav)).unwrap_err();
            assert!(
                err.to_string().contains("\\u{7}"),
                "expected an escaped byte in: {err}"
            );
        }

        #[test]
        fn invalid_utf8() {
            let wav = wav_with_guano(b"Make:\xff\xfe invalid\n");
            let err = GuanoFile::new(Cursor::new(wav)).unwrap_err();
            assert!(matches!(err, GuanoError::MalformedMetadata(_)));
        }

        /// A chunk running past EOF is a damaged file, and must not be reported as
        /// "this recorder wrote no GUANO metadata".
        #[test]
        fn truncated_chunk_is_not_reported_as_missing_metadata() {
            let mut wav = wav_with_guano(b"GUANO|Version:1.0\n");
            wav.truncate(wav.len() - 6); // chop the tail off the guan chunk
            let err = GuanoFile::new(Cursor::new(wav)).unwrap_err();
            let GuanoError::TruncatedFile {
                chunk,
                declared_size,
                available,
            } = err
            else {
                panic!("expected TruncatedFile, got {err:?}");
            };
            assert_eq!(chunk, "guan");
            assert_eq!(declared_size, 18);
            assert_eq!(available, 12);
        }
    }

    /// The serde implementations are hand written rather than derived, so the shape of the
    /// output and the ordering guarantees are not enforced by the compiler and need tests.
    #[cfg(feature = "serde")]
    mod serde_impls {
        use super::*;

        fn recording() -> GuanoFile {
            let f =
                File::open("testdata/recording.wav").expect("testdata/recording.wav is missing");
            GuanoFile::new(f).expect("recording.wav must parse")
        }

        /// A derived implementation would emit `{"String":"1.0"}` and leak the Rust enum into
        /// the output. GUANO values are plain text and must serialize as plain text.
        #[test]
        fn string_serializes_as_bare_json_string() {
            let value = GuanoValue::String("1.0".to_owned());
            assert_eq!(serde_json::to_string(&value).unwrap(), r#""1.0""#);
        }

        #[test]
        fn namespace_serializes_as_nested_object() {
            let mut ns = HashMap::new();
            ns.insert("Version".to_owned(), GuanoValue::String("1.0".to_owned()));
            let value = GuanoValue::Object(ns);
            assert_eq!(
                serde_json::to_string(&value).unwrap(),
                r#"{"Version":"1.0"}"#
            );
        }

        /// The spec requires `GUANO|Version` to be the first field, so its namespace leads.
        #[test]
        fn guano_namespace_is_the_first_key() {
            let json = serde_json::to_string(&recording()).unwrap();
            assert!(
                json.starts_with(r#"{"GUANO":{"Version":"#),
                "GUANO namespace should lead: {json}"
            );
        }

        #[test]
        fn remaining_root_keys_are_sorted() {
            let json = serde_json::to_string(&recording()).unwrap();
            // Alphabetical, and each pattern includes the colon so it can only match a key.
            let expected = [
                r#""Firmware Version":"#,
                r#""Length":"#,
                r#""Make":"#,
                r#""Model":"#,
                r#""Samplerate":"#,
                r#""Serial":"#,
                r#""Timestamp":"#,
            ];
            let positions: Vec<usize> = expected
                .iter()
                .map(|key| {
                    json.find(key)
                        .unwrap_or_else(|| panic!("missing key {key} in {json}"))
                })
                .collect();
            assert!(
                positions.windows(2).all(|w| w[0] < w[1]),
                "root keys are not alphabetical: {json}"
            );
        }

        /// `HashMap` iteration order varies per map instance, so identical metadata built by a
        /// different insertion order must still produce byte-identical output.
        #[test]
        fn output_is_stable_across_serializations() {
            let forward: GuanoFile = serde_json::from_str(
                r#"{"Make":"Wildlife Acoustics, Inc.","Model":"Song Meter Mini","GUANO":{"Version":"1.0"},"Serial":"2MA04827"}"#,
            )
            .unwrap();
            let reversed: GuanoFile = serde_json::from_str(
                r#"{"Serial":"2MA04827","GUANO":{"Version":"1.0"},"Model":"Song Meter Mini","Make":"Wildlife Acoustics, Inc."}"#,
            )
            .unwrap();

            assert_eq!(forward, reversed);
            assert_eq!(
                serde_json::to_string(&forward).unwrap(),
                serde_json::to_string(&reversed).unwrap()
            );
        }

        #[test]
        fn round_trips_through_json() {
            let original = recording();
            let json = serde_json::to_string(&original).unwrap();
            let restored: GuanoFile = serde_json::from_str(&json).unwrap();
            assert_eq!(original, restored);
        }

        /// Namespaces split on the first `|` only, so the remainder of the key is an ordinary
        /// field name that must survive a round trip with its pipe intact.
        #[test]
        fn preserves_pipes_in_nested_keys() {
            let json = serde_json::to_string(&recording()).unwrap();
            let restored: GuanoFile = serde_json::from_str(&json).unwrap();
            assert_eq!(
                restored.metadata()["WA"]["Song Meter|Prefix"],
                GuanoValue::String("2MA04827".to_owned())
            );
        }

        /// Every GUANO field is text. A JSON number is not a value this crate could ever have
        /// parsed out of a `guan` chunk, so accepting it would invent data.
        #[test]
        fn rejects_non_string_scalars() {
            let err = serde_json::from_str::<GuanoFile>(r#"{"Samplerate":22050}"#).unwrap_err();
            assert!(
                err.to_string().contains("invalid type"),
                "unhelpful error: {err}"
            );
        }

        /// GUANO is a flat `namespace|key: value` format, so a second level of nesting could
        /// never be written back out.
        #[test]
        fn rejects_nesting_deeper_than_one_level() {
            assert!(
                serde_json::from_str::<GuanoFile>(r#"{"WA":{"Song Meter":{"Prefix":"X"}}}"#)
                    .is_err()
            );
        }
    }
}
