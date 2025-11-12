use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek},
};
use thiserror::Error;
pub struct GuanoFile {
    file: File,
    wav_data_offset: usize,
    wav_data_size: usize,
    map: HashMap<String, String>,
}

/// This struct represents a single file's GUANO metadata.
impl GuanoFile {
    /// Create a new GuanoFile
    pub fn new(file: File) -> Result<Self, GuanoError> {
        let mut gf = GuanoFile {
            file,
            map: HashMap::new(),
        };
        gf.load()?;
        Ok(gf)
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
        let mut chunksz = 0usize;
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
            }
            // this is where the actual PCM data begins
            else if chunkid_buf == c"data".to_bytes() {
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
}

#[derive(Error, Debug)]
pub enum GuanoError {
    #[error("File IO Error")]
    FileIOError(#[from] io::Error),
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
}
