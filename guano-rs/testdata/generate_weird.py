#!/usr/bin/env python3
"""Generate `weird.wav`, the edge-case fixture for the GUANO parser.

The file is deliberately weird but *fully spec-legal*, so it must parse cleanly.
Do not hand-edit `weird.wav` -- change this script and re-run it:

    python3 guano-rs/testdata/generate_weird.py

Structural traps (RIFF level):
  * an ODD-sized chunk sits before `guan`, so a broken pad-byte skip desyncs the
    chunk walk and `guan` is never found
  * `guan` is not the last chunk -- `wamd` follows it

Payload traps (GUANO level), all legal per the spec:
  * NUL padding to an even length, inside the declared chunk size, exactly as the
    Wildlife Acoustics Song Meter Mini writes it. The spec says to pad "with
    whitespace", and defines whitespace to "include the non-printing ASCII bytes
    including null, CR, LF, space, tab, etc."
  * CRLF line ending, padded keys and values, a value containing colons, a value
    containing '|', multi-byte UTF-8, an empty value, a blank line, a line of only
    whitespace and NUL, and a NUL clinging to the end of a value.
"""

import struct
from pathlib import Path


def chunk(cid: bytes, body: bytes) -> bytes:
    """A RIFF chunk: id, little-endian size, body, and a pad byte if odd."""
    out = cid + struct.pack("<I", len(body)) + body
    return out + (b"\x00" if len(body) % 2 else b"")


payload = (
    "GUANO|Version:1.0\n"
    "  Make  :  Wildlife Acoustics, Inc.  \n"  # padded key AND value
    "Model:Song Meter Mini\n"
    "Serial:2LA03876\n"
    "Timestamp:2026-01-15 08:12:02+01:00\n"  # colons inside the value
    "Samplerate:22050\n"
    "Temperature Int:-1.25\n"  # negative number
    "Loc Position:47.39861 7.67191\n"
    "WA|Song Meter|Prefix:TANIAULMET\n"  # '|' past the first one
    'WA|Song Meter|Audio settings:[{"rate":22050,"gain":18}]\n'  # 2nd key, same ns
    "Note:Waldkauz – Käuzchen, 5 °C\n"  # multi-byte UTF-8
    "Escaped:line one\\nline two\n"  # literal backslash-n, NOT unescaped
    "Empty Value:\n"  # value is the empty string
    "\n"  # blank line -> skipped
    "   \t \x00  \n"  # whitespace+NUL-only line -> skipped
    "Original Filename:weird.wav\r\n"  # CRLF ending
    "Trailing NUL Value:22050\x00\n"  # NUL clinging to a value
).encode("utf-8")

# Pad the metadata to an even length with NUL, as the Song Meter Mini does. The
# padding is part of the declared `guan` size, not a RIFF pad byte.
payload += b"\x00" * (len(payload) % 2)

body = (
    # PCM, mono, 22050 Hz, 16-bit
    chunk(b"fmt ", struct.pack("<HHIIHH", 1, 1, 22050, 44100, 2, 16))
    + chunk(b"junk", b"\x01\x02\x03\x04\x05")  # ODD size -> forces the RIFF pad path
    + chunk(b"data", b"\x00" * 8)
    + chunk(b"guan", payload)
    + chunk(b"wamd", b"\x00" * 4)  # a chunk AFTER guan
)

out = b"RIFF" + struct.pack("<I", 4 + len(body)) + b"WAVE" + body
dest = Path(__file__).with_name("weird.wav")
dest.write_bytes(out)
print(f"wrote {dest} ({len(out)} bytes, guan payload {len(payload)} bytes)")
