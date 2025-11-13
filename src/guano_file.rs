use core::hash;
use std::{
    collections::HashMap,
    fs::File,
    hash::Hash,
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
/// ```no_run
/// use std::fs::File;
/// use guano_rs::guano_file::GuanoFile;
///
/// let file = File::open("recording.wav").unwrap();
/// let guano = GuanoFile::new(file).unwrap();
/// let metadata = guano.metadata();
/// ```
pub struct GuanoFile {
    file: File,
    wav_data_offset: usize,
    wav_data_size: usize,
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
/// ```no_run
/// use guano_rs::guano_file::{GuanoFile, GuanoValue};
/// use std::fs::File;
///
/// let file = File::open("recording.wav").unwrap();
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
pub enum GuanoValue {
    /// A simple string value
    String(String),
    /// A nested object containing namespaced metadata fields
    Object(HashMap<String, GuanoValue>),
}

impl Index<&str> for GuanoValue {
    type Output = GuanoValue;

    fn index(&self, index: &str) -> &Self::Output {
        match self {
            GuanoValue::String(_) => panic!(),
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
    /// ```no_run
    /// use std::fs::File;
    /// use guano_rs::guano_file::GuanoFile;
    ///
    /// let file = File::open("bat_recording.wav")?;
    /// let guano = GuanoFile::new(file)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(file: File) -> Result<Self, GuanoError> {
        let mut gf = GuanoFile {
            file,
            wav_data_offset: 0,
            wav_data_size: 0,
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
    /// ```no_run
    /// use std::fs::File;
    /// use guano_rs::guano_file::{GuanoFile, GuanoValue};
    ///
    /// let file = File::open("recording.wav")?;
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
        buf_reader.seek(io::SeekFrom::Start(0x0C))?;
        loop {
            // read chunk id big endian
            buf_reader.read_exact(&mut chunkid_buf)?;
            // read chunk size as little endian
            buf_reader.read_exact(&mut chunksz_buf)?;
            chunksz = u32::from_le_bytes(chunksz_buf) as usize;

            // if "guan" is present, extract the metadata
            if chunkid_buf == c"guan".to_bytes() {
                let mut metadata_buf = vec![0; chunksz];
                buf_reader.read_exact(&mut metadata_buf[0..chunksz])?;
                drop(buf_reader);
                self.parse(&metadata_buf);
                break;
            }
            // this is where the actual PCM data begins
            else if chunkid_buf == c"data".to_bytes() {
                self.wav_data_offset = buf_reader.stream_position()? as usize;
                self.wav_data_size = chunksz;
                buf_reader.seek_relative(chunksz.try_into().unwrap())?;
            } else {
                buf_reader.seek_relative(chunksz.try_into().unwrap())?;
            }
            if chunksz % 2 != 0 {
                buf_reader.seek_relative(1)?;
            }
        }

        Ok(())
    }

    fn parse(&mut self, raw: &[u8]) {
        // check if the str can be interpreted directly
        if let Ok(str) = str::from_utf8(&raw) {
            for mut line in str.lines() {
                line = line.trim();
                let kv: Vec<&str> = line.splitn(2, ':').collect();
                assert_eq!(kv.len(), 2);
                let full_key = kv[0];
                let val = kv[1].to_owned();
                // check to see if the key has a namespace
                if full_key.contains('|') {
                    let ns: Vec<&str> = full_key.splitn(2, '|').collect();
                    assert_eq!(ns.len(), 2);
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
        }
        // lossy interpretation of the GUANO metadata
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_file() -> Result<(), GuanoError> {
        let f = File::open("testdata/2MA04827_20250227_063900.wav")?;
        GuanoFile::new(f)?;
        Ok(())
    }

    #[test]
    fn has_guano_version() -> Result<(), GuanoError> {
        let f = File::open("testdata/2MA04827_20250227_063900.wav")?;
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
}
