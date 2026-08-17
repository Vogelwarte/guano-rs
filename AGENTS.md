# AGENTS.md

Notes for agents working on `guano-rs`. Read the "GUANO rules that bite" section before touching
the parser — every rule there is one somebody already got wrong.

## What this is

An **independent** reader for GUANO (Grand Unified Acoustic Notation Ontology) metadata embedded in
the `guan` RIFF chunk of WAV files, used for bat and other passive acoustic recordings. Not
affiliated with the reference implementation.

It is **read-only with respect to WAV files**: there is no way to write a `guan` chunk, and no
mutation or setter API — `GuanoFile::map` is private. Don't assume a write path exists.

Metadata that has been read *can* leave the process: `GuanoFile` and `GuanoValue` implement serde's
`Serialize`/`Deserialize` (hand written, not derived — see Serde below).

Rust **edition 2024**, Cargo workspace with `resolver = "3"`.

## Features

| Feature | Default | Pulls in | Gates |
|---|---|---|---|
| `cli` | yes | `clap`, `serde_json` | the `guano-rs` binary, via `required-features` on `[[bin]]` |
| `serde` | via `cli` | `serde` | the `Serialize`/`Deserialize` impls, `sorted_keys`/`ordered_keys`, and `mod serde_impls` |

The library itself needs only `thiserror`. `clap` alone drags in ~13 crates for a binary that
library consumers never build, which is why `guano-wasm` sets `default-features = false` — that
took its build from 44 crates to 24. **Keep it that way**; a stray default-featured dependency
edge puts clap back in the wasm build.

`serde_json` is *not* a library dependency: `lib.rs` uses it only in `#[cfg(test)]` code and doc
examples, so it is both an optional dependency (for the bin) and a dev-dependency (for tests under
`--no-default-features --features serde`).

`serde` is declared **without** its `derive` feature on purpose — see Serde below.

Check all three combinations when touching `Cargo.toml` or the `#[cfg(feature = "serde")]`
attributes:

```sh
cargo test -p guano-rs                                          # 27 tests + 9 doc tests
cargo test -p guano-rs --no-default-features                    # 18 tests + 5 doc tests
cargo test -p guano-rs --no-default-features --features serde   # 27 tests + 9 doc tests
```

## Layout

| Path | What |
|---|---|
| [guano-rs/src/lib.rs](guano-rs/src/lib.rs) | The whole library: `GuanoFile`, `GuanoValue`, `GuanoError`. RIFF walk in `load()`, metadata parse in `parse()`, serde impls after the `Index` impl. |
| [guano-rs/src/main.rs](guano-rs/src/main.rs) | `guano-rs` CLI (clap). `--format kv\|json`, `--compact`. |
| [guano-wasm/src/lib.rs](guano-wasm/src/lib.rs) | `wasm-bindgen` wrapper, `cdylib`, published as a scoped npm package to the GitLab registry. |
| [guano-rs/testdata/](guano-rs/testdata/) | Fixtures — see Testing. |

**Design invariant:** `GuanoFile::new` is generic over `Read + Seek`, which is *why* the wasm
wrapper can hand it a `Cursor` over browser-read bytes. Do not narrow it to `File`.

## GUANO rules that bite

Source of truth: the
[GUANO specification](https://github.com/riggsd/guano-spec/blob/master/guano_specification.md).
Consult it before "improving" the parser.

- **Whitespace is wider than Rust's.** The spec: whitespace "should include the non-printing ASCII
  bytes including null, CR, LF, space, tab, etc." Rust's `str::trim` follows the Unicode
  `White_Space` property, which **excludes NUL**. Use `trim_guano()`, never bare `.trim()`, on any
  GUANO text. Getting this wrong rejected entire valid recordings in production.
- **Padding is whitespace padding, and NUL counts.** Sub-chunks are padded to an even byte length
  "with whitespace"; the Wildlife Acoustics Song Meter Mini pads with NUL, *inside* the declared
  `guan` size. Such a file is **valid**. Never add a "reject NUL as corruption" check.
- **Namespaces split on the first `|` only.** `WA|Song Meter|Prefix` becomes
  `map["WA"]["Song Meter|Prefix"]` — the remainder is the field name, not a deeper nesting. This is
  correct; the wasm tests assert it. Do not "fix" it.
- **Values keep their colons.** The key ends at the *first* `:`; everything after it is the value.
- **Encoding is UTF-8**, always ("All GUANO metadata must be persisted as UTF-8 Unicode string").
  Invalid UTF-8 must stay a hard error — never swap in a lossy decode.
- **Known gap:** a literal newline inside a value is written as the two characters `\n`, and the
  parser does *not* unescape them yet. `testdata/weird.wav` pins the current behaviour, so change
  that test deliberately if you implement unescaping.

## Leniency policy

Recorders in the field do emit genuinely corrupt files, and those **must keep failing loudly** — a
silently half-parsed file is worse than a rejected one, because it quietly poisons downstream data.

Widen the parser only where the spec says to. Specifically:

- **Never** add a blanket "skip lines that don't parse".
- A line without a `:`, invalid UTF-8, and a chunk running past EOF are all hard errors.
- `GuanoError::TruncatedFile` is deliberately distinct from `NoGuanoMetadata`: "this file is
  damaged" and "this recorder wrote no GUANO" demand different responses when triaging a season of
  recordings. Don't collapse them.
- Error messages carry the line number and escape non-printing bytes (`escape_debug`). Keep that —
  an unescaped NUL renders as nothing and makes a report undiagnosable.

The tests under `mod still_rejects` enforce all of this. If one starts failing, the change is wrong.

## Serde

Gated behind the `serde` feature (on by default via `cli`). The impls are **hand written on
purpose** — which is also why `serde` is declared without its `derive` feature; there is not a
single `#[derive(Serialize)]` in the repo. Three things will break if someone "simplifies" them to
a derive:

- **Shape.** A derived `Serialize` on the `GuanoValue` enum emits the externally tagged
  `{"String": "1.0"}`, leaking the Rust enum into the output. GUANO values are plain text.
  `#[serde(untagged)]` fixes the shape but not the next point.
- **Ordering.** `HashMap` iteration order varies per map instance, so a derive emits the same file
  differently on every run. `ordered_keys()` puts the `GUANO` namespace first (the spec requires
  `GUANO|Version` to be the first field) and sorts the rest; `sorted_keys()` handles nested
  namespaces. `mod serde_impls` pins this, and `print_kv` in `main.rs` mirrors the same order so
  both CLI formats agree.
- **Strictness.** `GuanoValue`'s visitor reads namespace entries as `next_entry::<String, String>()`.
  That one type parameter is what rejects non-string scalars and nesting deeper than one level —
  neither could ever be written back into a `guan` chunk, since GUANO is a flat
  `namespace|key: value` format. Deserializing uses `deserialize_any`, so self-describing formats
  only.

`GuanoFile` serializes as the metadata map itself, not as a wrapper object around it. `Deserialize`
for `GuanoFile` is *not* a mutation API: it builds a detached metadata map that cannot be written to
a WAV.

## Testing

```sh
cargo test                  # lib + doc tests
cargo build --release
cd guano-wasm && wasm-pack test --release --node
```

Fixtures in [guano-rs/testdata/](guano-rs/testdata/):

| File | Purpose |
|---|---|
| `recording.wav` | An ordinary Song Meter recording. |
| `no_meta.wav` | Valid WAV, no `guan` chunk → `NoGuanoMetadata`. |
| `smol.wav` | Too short for a RIFF header → `FileHeaderError`. |
| `weird.wav` | Deliberately weird but **fully spec-legal**, so it must parse. |

`weird.wav` is generated by [generate_weird.py](guano-rs/testdata/generate_weird.py) — **edit the
script and re-run it, never hand-edit the binary**. It packs the edge cases into one file: NUL
padding inside the declared chunk size, an odd-sized chunk before `guan` (so a broken RIFF pad-byte
skip desynchronizes the walk), `guan` not being the last chunk, CRLF endings, padded keys and
values, colons and `|` inside values, multi-byte UTF-8, an empty value, a blank line, and a
whitespace-plus-NUL line. New spec-legal edge cases belong in that file and its generator rather
than in a new fixture.

Negative cases build WAVs in memory with `Cursor<Vec<u8>>` instead of committing a file per variant.
If you copy the wasm helper's approach, write chunk sizes as `(len as u32).to_le_bytes()` — the wasm
test's `usize::to_le_bytes()` only works because it compiles for 32-bit wasm and would panic on a
64-bit native target.

**Doc examples are executed** by `cargo test` and open the relative path `testdata/recording.wav`
(CWD is the crate root). Keep them runnable when editing rustdoc.

## CI

GitLab, not GitHub — [.gitlab-ci.yml](.gitlab-ci.yml), hosted at `gitlab.vogelwarte.ch`. These block
the pipeline, so run them before proposing a change:

```sh
cargo fmt --all -- --check
cargo clippy -- -D warnings
```

Note the clippy gate omits `--all-targets`, so lints inside `#[cfg(test)]` code do not fail CI. Run
`cargo clippy --all-targets -- -D warnings` locally anyway.

Later stages build the release binary and docs, build the wasm package (stamping the version with
the short commit SHA), and publish the npm package to the GitLab registry on `main` and tags.

## Style

`thiserror` for the error enum; every public item carries rustdoc with a runnable example. Match
that density when adding public API.

## Housekeeping quirks

- A stale `guano-rs/Cargo.lock` sits beside the workspace root `Cargo.lock`; the workspace uses the
  root one.
- `guano-rs/Cargo.toml` says `0.1.0` while the only git tag is `0.0.1`.
- `guano-wasm/Cargo.toml` defines `[profile.release]`, which Cargo ignores for non-root packages and
  warns about on every build.
- Production recordings run 150 MB+ and live on a slow DFS network share, so the RIFF walk seeks
  rather than reading; keep it that way.
