# guano-rs

A Rust library for reading GUANO metadata from WAV files.

## About

GUANO (Grand Unified Acoustic Notation Ontology) is a metadata format for bat acoustic recordings stored within WAV files. This library provides a simple interface to read and parse GUANO metadata from WAV files.

**Note:** This is an independent implementation and is not affiliated with the authors of the reference GUANO implementation. Currently, this library only supports reading metadata, not writing it.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
guano-rs = "0.1.0"
```

## Usage

### Basic Example

```rust
use std::fs::File;
use guano_rs::GuanoFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open a WAV file containing GUANO metadata
    let file = File::open("path/to/your/file.wav")?;

    // Parse the GUANO metadata
    let guano_file = GuanoFile::new(file)?;

    // Access the metadata
    let metadata = guano_file.metadata();

    // Access root-level fields
    if let Some(timestamp) = metadata.get("Timestamp") {
        println!("Timestamp: {:?}", timestamp);
    }

    // Access namespaced fields
    if let Some(guano_namespace) = metadata.get("GUANO") {
        println!("GUANO Version: {:?}", guano_namespace["Version"]);
    }

    Ok(())
}
```

### Accessing Metadata

The metadata is organized as a `HashMap<String, GuanoValue>` where `GuanoValue` can be either:
- `GuanoValue::String(String)` - A simple string value
- `GuanoValue::Object(HashMap<String, GuanoValue>)` - A nested object for namespaced metadata

You can access nested metadata using the index operator:

```rust
// Access nested metadata
let version = &guano_file.metadata()["GUANO"]["Version"];
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## References

- [GUANO Specification](https://github.com/riggsd/guano-spec/blob/master/guano_specification.md)
