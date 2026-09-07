# guano-wasm

WebAssembly bindings for reading GUANO metadata from WAV files in JavaScript and TypeScript.

## About

This package provides WebAssembly bindings to [guano](../guano), allowing you to read GUANO (Grand Unified Acoustic Notation Ontology) metadata from bat acoustic recordings in browser and Node.js environments.

GUANO is a metadata format for bat acoustic recordings stored within WAV files. This library provides a simple JavaScript/TypeScript interface to read and parse GUANO metadata.

**Note:** This is an independent implementation and is not affiliated with the authors of the reference GUANO implementation. Currently, this library only supports reading metadata, not writing it.

## Installation

```bash
npm install @vogelwarte.ch/guano-wasm
```

## Usage

### Basic Example (Node.js)

```javascript
import { GuanoFile } from '@vogelwarte.ch/guano-wasm';
import { readFileSync } from 'fs';

// Read WAV file as a byte array
const bytes = readFileSync('path/to/your/file.wav');

// Parse the GUANO metadata
const guanoFile = new GuanoFile(bytes);

// Access root-level metadata
const timestamp = guanoFile.get_metadata('Timestamp');
console.log('Timestamp:', timestamp);

const model = guanoFile.get_metadata('Model');
console.log('Model:', model);

// Access namespaced metadata
const version = guanoFile.get_metadata_inside_namespace('GUANO', 'Version');
console.log('GUANO Version:', version);
```

### Basic Example (Browser)

```javascript
import { GuanoFile } from '@vogelwarte.ch/guano-wasm';

async function loadGuanoFile(file) {
  // Read file as ArrayBuffer
  const arrayBuffer = await file.arrayBuffer();
  const bytes = new Uint8Array(arrayBuffer);

  // Parse the GUANO metadata
  const guanoFile = new GuanoFile(bytes);

  // Access metadata
  const timestamp = guanoFile.get_metadata('Timestamp');
  console.log('Timestamp:', timestamp);

  return guanoFile;
}

// Use with file input
document.getElementById('fileInput').addEventListener('change', async (e) => {
  const file = e.target.files[0];
  const guanoFile = await loadGuanoFile(file);
});
```

### TypeScript Example

```typescript
import { GuanoFile } from '@vogelwarte.ch/guano-wasm';
import { readFileSync } from 'fs';

const bytes: Uint8Array = readFileSync('path/to/your/file.wav');

try {
  const guanoFile: GuanoFile = new GuanoFile(bytes);

  // Check if metadata exists before accessing
  if (guanoFile.metadata_contains('Timestamp')) {
    const timestamp: string | undefined = guanoFile.get_metadata('Timestamp');
    console.log('Timestamp:', timestamp);
  }

  // Check namespaced metadata
  if (guanoFile.metadata_namespace_contains('GUANO', 'Version')) {
    const version: string | undefined =
      guanoFile.get_metadata_inside_namespace('GUANO', 'Version');
    console.log('GUANO Version:', version);
  }

} catch (error) {
  console.error('Failed to parse GUANO metadata:', error);
}
```

## API Reference

### `GuanoFile`

#### Constructor

```typescript
new GuanoFile(bytes: Uint8Array): GuanoFile
```

Creates a new GuanoFile instance from a byte array. Throws an error if the file cannot be parsed.

**Parameters:**
- `bytes` - The WAV file contents as a Uint8Array

**Returns:** A new `GuanoFile` instance

**Throws:** String error message if parsing fails

#### Methods

##### `get_metadata(key: string): string | undefined`

Get metadata value for a key in the root namespace.

**Parameters:**
- `key` - The metadata key to retrieve

**Returns:** The metadata value as a string, or `undefined` if not found

##### `get_metadata_inside_namespace(namespace: string, key: string): string | undefined`

Get metadata value for a key within a specific namespace.

**Parameters:**
- `namespace` - The namespace name (e.g., "GUANO", "WA")
- `key` - The metadata key to retrieve within the namespace

**Returns:** The metadata value as a string, or `undefined` if not found

##### `metadata_keys(): string[]`

Returns all metadata keys.

- Root namespace keys are returned as-is (e.g., `"Timestamp"`)
- Namespaced keys are returned in the format `"namespace|key"` (e.g., `"GUANO|Version"`, `"WA|Song Meter|Prefix"`)

**Returns:** Array of all metadata keys

##### `metadata_contains(key: string): boolean`

Check if a key exists in the root namespace.

**Parameters:**
- `key` - The metadata key to check

**Returns:** `true` if the key exists, `false` otherwise

##### `metadata_namespace_contains(namespace: string, key: string): boolean`

Check if a key exists within a specific namespace.

**Parameters:**
- `namespace` - The namespace name
- `key` - The metadata key to check

**Returns:** `true` if the key exists in the namespace, `false` otherwise

## Common GUANO Metadata Fields

Some common metadata fields you might encounter:

**Root namespace:**
- `Timestamp` - Recording timestamp in ISO 8601 format
- `Model` - Recording device model
- `Make` - Device manufacturer
- `Serial` - Device serial number
- `Samplerate` - Audio sample rate in Hz
- `Length` - Recording length in seconds
- `Loc Position` - GPS coordinates (latitude longitude)
- `Temperature Int` - Internal temperature in Celsius

**GUANO namespace:**
- `GUANO|Version` - GUANO specification version

**Device-specific namespaces:**
- `WA|*` - Wildlife Acoustics specific metadata
- And others depending on the recording device

## License

This project is licensed under the MIT License - see [LICENSE](./LICENSE).

## References

- [GUANO Specification](https://github.com/riggsd/guano-spec/blob/master/guano_specification.md)
