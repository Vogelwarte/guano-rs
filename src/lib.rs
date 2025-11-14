///
/// This is an independant implementation for reading and writing GUANO metadata.
/// The authors of this package are not associated with the authors of the reference implementation.
///
///
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader, Read, Seek},
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
/// use guano_rs::GuanoFile;
///
/// let file = File::open("testdata/recording.wav").unwrap();
/// let guano = GuanoFile::new(file).unwrap();
/// let metadata = guano.metadata();
/// ```
#[derive(Debug)]
pub struct GuanoFile {
    file: File,
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
/// use guano_rs::{GuanoFile, GuanoValue};
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
#[derive(Debug)]
pub enum GuanoValue {
    /// A simple string value
    String(String),
    /// A nested object containing namespaced metadata fields
    Object(HashMap<String, GuanoValue>),
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
    /// use guano_rs::{GuanoFile, GuanoValue};
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
    /// use guano_rs::GuanoFile;
    ///
    /// let file = File::open("testdata/recording.wav")?;
    /// let guano = GuanoFile::new(file)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(file: File) -> Result<Self, GuanoError> {
        let mut gf = GuanoFile {
            file,
            map: HashMap::new(),
        };
        gf.load()?;
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
    /// use guano_rs::{GuanoFile, GuanoValue};
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

    fn load(&mut self) -> Result<(), GuanoError> {
        // check the file size. A valid RIFF header must be at least 8 bytes in size
        if self.file.metadata()?.len() < 8 {
            return Err(GuanoError::FileHeaderError(
                "File too small to contain RIFF \"WAVE\" header".to_owned(),
            ));
        }
        let mut buf_reader = BufReader::new(&self.file);
        // check that the file contains the "WAVE" RIFF chunk at 0x08
        buf_reader.seek(io::SeekFrom::Start(0x08))?;
        let mut header = [0u8; 4];
        buf_reader.read_exact(&mut header)?;
        if header != c"WAVE".to_bytes() {
            return Err(GuanoError::FileHeaderError(format!(
                "Expecxted RIFF chunk \"WAVE\", but found {}",
                String::from_utf8_lossy(&header)
            )));
        }

        let mut chunkid_buf = [0u8; 4];
        let mut chunksz_buf = [0u8; 4];
        let mut chunksz;
        let mut found_guano = false;
        buf_reader.seek(io::SeekFrom::Start(0x0C))?;
        loop {
            // read chunk id big endian
            if buf_reader.read_exact(&mut chunkid_buf).is_err() {
                // Reached end of file
                break;
            }
            // read chunk size as little endian
            buf_reader.read_exact(&mut chunksz_buf)?;
            chunksz = u32::from_le_bytes(chunksz_buf) as usize;

            // if "guan" is present, extract the metadata
            if chunkid_buf == c"guan".to_bytes() {
                let mut metadata_buf = vec![0; chunksz];
                buf_reader.read_exact(&mut metadata_buf[0..chunksz])?;
                drop(buf_reader);
                self.parse(&metadata_buf)?;
                found_guano = true;
                break;
            } else {
                buf_reader.seek_relative(chunksz.try_into().unwrap())?;
            }
            if chunksz % 2 != 0 {
                buf_reader.seek_relative(1)?;
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

        for mut line in str.lines() {
            line = line.trim();
            if line.is_empty() {
                continue; // Skip empty lines
            }

            let kv: Vec<&str> = line.splitn(2, ':').collect();
            if kv.len() != 2 {
                return Err(GuanoError::MalformedMetadata(format!(
                    "Expected key:value format, found: '{}'",
                    line
                )));
            }

            let full_key = kv[0];
            let val = kv[1].to_owned();

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
    #[error("RIFF \"WAVE\" header error")]
    FileHeaderError(String),

    /// The WAV file does not contain a GUANO metadata chunk.
    ///
    /// This error occurs when the file is a valid WAV file but does not contain
    /// the `guan` RIFF chunk that stores GUANO metadata.
    #[error("No GUANO metadata chunk found in file")]
    NoGuanoMetadata,

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
}
