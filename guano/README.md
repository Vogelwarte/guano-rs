# guano

A Rust library for reading GUANO metadata from WAV files.

## About

GUANO (Grand Unified Acoustic Notation Ontology) is a metadata format for bat acoustic recordings stored within WAV files. This library provides a simple interface to read and parse GUANO metadata from WAV files.

**Note:** This is an independent implementation and is not affiliated with the authors of the reference GUANO implementation. Currently, this library only supports reading metadata from WAV files, not writing it back to them. Parsed metadata can, however, be serialized to JSON and other [serde](https://serde.rs) formats — see [Serialization](#serialization).

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
guano = { version = "0.1.1", default-features = false, features = ["serde"] }
```

### Feature flags

| Feature | Default | Description |
|---|---|---|
| `cli` | yes | Builds the `guano` command line tool. Pulls in `clap` and `serde_json`. |
| `serde` | via `cli` | `Serialize`/`Deserialize` for `GuanoFile` and `GuanoValue`. Pulls in `serde`. |

The default features build the command line tool. As a library you usually want
`default-features = false`, which leaves `thiserror` as the only dependency, and then `serde` if
you need [serialization](#serialization):

```toml
# reading only, minimal dependencies
guano = { version = "0.1.1", default-features = false }
```

## Usage

### Basic Example

```rust
use std::fs::File;
use guano::GuanoFile;

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

## Serialization

*Requires the `serde` feature, which is enabled by default.*

`GuanoFile` and `GuanoValue` implement serde's `Serialize` and `Deserialize`, so metadata can be
converted to JSON (or any other serde format) and read back:

```rust
use std::fs::File;
use guano::GuanoFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guano_file = GuanoFile::new(File::open("path/to/your/file.wav")?)?;

    let json = serde_json::to_string_pretty(&guano_file)?;
    println!("{}", json);

    let restored: GuanoFile = serde_json::from_str(&json)?;
    assert_eq!(guano_file, restored);

    Ok(())
}
```

A file serializes as a flat mapping of field names to values, with namespaced fields nested under
their namespace:

```json
{
  "GUANO": {
    "Version": "1.0"
  },
  "Make": "Wildlife Acoustics, Inc.",
  "Model": "Song Meter Mini",
  "WA": {
    "Song Meter|Prefix": "2MA04827"
  }
}
```

The output is deterministic: the `GUANO` namespace comes first, as the specification requires
`GUANO|Version` to be the first field, and the remaining keys follow alphabetically. Serializing the
same metadata twice always produces identical bytes, so output can be diffed or checksummed.

Because every GUANO field is text, deserialization accepts only strings and one level of namespace
nesting. A JSON number, or an object nested two levels deep, is rejected rather than silently
coerced.

## Command line tool

*Requires the `cli` feature, which is enabled by default.*

The crate ships a small binary for inspecting a file:

```sh
guano recording.wav                    # key-value output
guano recording.wav --format json      # pretty-printed JSON
guano recording.wav --format json -c   # compact JSON
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## References

- [GUANO Specification](https://github.com/riggsd/guano-spec/blob/master/guano_specification.md)
