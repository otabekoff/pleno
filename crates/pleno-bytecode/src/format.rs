//! The on-disk format of compiled bytecode (`.plc` files).
//!
//! A `.plc` file is an 8-byte header followed by the bincode-encoded [`Chunk`]:
//!
//! | offset | size | contents                                              |
//! |--------|------|-------------------------------------------------------|
//! | 0      | 4    | magic bytes [`BYTECODE_MAGIC`] (`PLNC`)               |
//! | 4      | 4    | [`BYTECODE_FORMAT_VERSION`] as a little-endian `u32`  |
//! | 8      | rest | the chunk, encoded with bincode 1 (fixed-width ints)  |
//!
//! Readers check the magic and the version before decoding the payload, so a
//! file written by a different `pleno` is rejected with a clear error instead
//! of being misread.

use std::fmt;

use bincode::Options;

use crate::Chunk;

/// The first four bytes of every `.plc` file.
pub const BYTECODE_MAGIC: [u8; 4] = *b"PLNC";

/// The version of the serialized bytecode format.
///
/// Bump this whenever the serialized form of [`Chunk`] changes: a new, removed
/// or reordered field or opcode, a change to `Value`, `FunctionProto` or any
/// other type stored in a chunk, or a change to the bincode options below.
/// Files with any other version are rejected on load.
pub const BYTECODE_FORMAT_VERSION: u32 = 5;

/// Size of the header in bytes: the magic followed by the version.
pub const BYTECODE_HEADER_LEN: usize = 8;

/// Why a `.plc` file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BytecodeFormatError {
    /// The file does not start with [`BYTECODE_MAGIC`].
    NotBytecode,
    /// The file was written with another [`BYTECODE_FORMAT_VERSION`].
    VersionMismatch { found: u32 },
    /// The magic is present but the version is cut off.
    TruncatedHeader,
    /// The payload after the header could not be decoded.
    CorruptPayload(String),
    /// The chunk could not be encoded.
    Encode(String),
}

impl fmt::Display for BytecodeFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotBytecode => write!(f, "not a Pleno bytecode file"),
            Self::VersionMismatch { found } => write!(
                f,
                "this .plc file was compiled by a different Pleno version (bytecode format \
                 {found}, this pleno reads {BYTECODE_FORMAT_VERSION}); recompile it with \
                 `pleno compile`"
            ),
            Self::TruncatedHeader => write!(
                f,
                "this .plc file is truncated (its header is incomplete); recompile it with \
                 `pleno compile`"
            ),
            Self::CorruptPayload(err) => write!(
                f,
                "this .plc file is truncated or corrupt ({err}); recompile it with `pleno compile`"
            ),
            Self::Encode(err) => write!(f, "failed to encode bytecode: {err}"),
        }
    }
}

impl std::error::Error for BytecodeFormatError {}

/// The bincode configuration of the payload. It matches `bincode::serialize`
/// (fixed-width little-endian integers) but rejects trailing bytes on read.
fn payload_options() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
}

/// The header written at the start of every `.plc` file.
pub fn bytecode_header() -> [u8; BYTECODE_HEADER_LEN] {
    let mut header = [0; BYTECODE_HEADER_LEN];
    header[..4].copy_from_slice(&BYTECODE_MAGIC);
    header[4..].copy_from_slice(&BYTECODE_FORMAT_VERSION.to_le_bytes());
    header
}

/// Checks the header of `bytes` and returns the payload that follows it.
pub fn check_bytecode_header(bytes: &[u8]) -> Result<&[u8], BytecodeFormatError> {
    if bytes.len() < BYTECODE_MAGIC.len() || bytes[..4] != BYTECODE_MAGIC {
        return Err(BytecodeFormatError::NotBytecode);
    }
    if bytes.len() < BYTECODE_HEADER_LEN {
        return Err(BytecodeFormatError::TruncatedHeader);
    }
    let found = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if found != BYTECODE_FORMAT_VERSION {
        return Err(BytecodeFormatError::VersionMismatch { found });
    }
    Ok(&bytes[BYTECODE_HEADER_LEN..])
}

/// Encodes `chunk` as the contents of a `.plc` file: header, then payload.
pub fn encode_chunk(chunk: &Chunk) -> Result<Vec<u8>, BytecodeFormatError> {
    let payload = payload_options()
        .serialize(chunk)
        .map_err(|err| BytecodeFormatError::Encode(err.to_string()))?;
    let mut bytes = Vec::with_capacity(BYTECODE_HEADER_LEN + payload.len());
    bytes.extend_from_slice(&bytecode_header());
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

/// Decodes the contents of a `.plc` file, checking the header first.
pub fn decode_chunk(bytes: &[u8]) -> Result<Chunk, BytecodeFormatError> {
    let payload = check_bytecode_header(bytes)?;
    // Decoding from a slice bounds every length by the bytes that are left, so a
    // damaged length field fails instead of allocating without limit.
    payload_options()
        .deserialize(payload)
        .map_err(|err| BytecodeFormatError::CorruptPayload(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Instruction, Opcode};
    use pleno_value::Value;

    fn sample_chunk() -> Chunk {
        let mut chunk = Chunk::new();
        chunk.constants.push(Value::Int(42));
        chunk.code.push(Instruction::new(Opcode::Return, 0, 1, 0));
        chunk.spans.push(pleno_common::Span::new(0, 1));
        chunk.lines.push(1);
        chunk.max_registers = 1;
        chunk.module_name = Some("main".to_string());
        chunk
    }

    #[test]
    fn header_layout() {
        let header = bytecode_header();
        assert_eq!(&header[..4], b"PLNC");
        assert_eq!(&header[4..], &BYTECODE_FORMAT_VERSION.to_le_bytes());
    }

    #[test]
    fn round_trip() {
        let chunk = sample_chunk();
        let bytes = encode_chunk(&chunk).unwrap();
        assert_eq!(&bytes[..BYTECODE_HEADER_LEN], &bytecode_header());
        // The payload after the header is plain `bincode::serialize` output.
        assert_eq!(
            &bytes[BYTECODE_HEADER_LEN..],
            bincode::serialize(&chunk).unwrap().as_slice()
        );
        let decoded = decode_chunk(&bytes).unwrap();
        assert_eq!(decoded.code, chunk.code);
        assert_eq!(decoded.lines, chunk.lines);
        assert_eq!(decoded.max_registers, chunk.max_registers);
        assert_eq!(decoded.module_name, chunk.module_name);
        assert!(matches!(decoded.constants[0], Value::Int(42)));
    }

    #[test]
    fn wrong_magic() {
        let chunk = sample_chunk();
        // A headerless file from an older pleno.
        let old = bincode::serialize(&chunk).unwrap();
        assert_eq!(
            decode_chunk(&old).unwrap_err(),
            BytecodeFormatError::NotBytecode
        );
        assert_eq!(
            decode_chunk(b"").unwrap_err(),
            BytecodeFormatError::NotBytecode
        );
        assert_eq!(
            decode_chunk(b"PLN").unwrap_err(),
            BytecodeFormatError::NotBytecode
        );
        let err = decode_chunk(b"print(1)\n").unwrap_err();
        assert_eq!(err.to_string(), "not a Pleno bytecode file");
    }

    #[test]
    fn wrong_version() {
        let mut bytes = encode_chunk(&sample_chunk()).unwrap();
        let other = BYTECODE_FORMAT_VERSION + 1;
        bytes[4..8].copy_from_slice(&other.to_le_bytes());
        let err = decode_chunk(&bytes).unwrap_err();
        assert_eq!(err, BytecodeFormatError::VersionMismatch { found: other });
        assert_eq!(
            err.to_string(),
            format!(
                "this .plc file was compiled by a different Pleno version (bytecode format \
                 {other}, this pleno reads {BYTECODE_FORMAT_VERSION}); recompile it with \
                 `pleno compile`"
            )
        );
    }

    #[test]
    fn truncated_file() {
        let bytes = encode_chunk(&sample_chunk()).unwrap();
        for len in 4..BYTECODE_HEADER_LEN {
            assert_eq!(
                decode_chunk(&bytes[..len]).unwrap_err(),
                BytecodeFormatError::TruncatedHeader
            );
        }
        for len in BYTECODE_HEADER_LEN..bytes.len() {
            assert!(matches!(
                decode_chunk(&bytes[..len]),
                Err(BytecodeFormatError::CorruptPayload(_))
            ));
        }
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let mut bytes = encode_chunk(&sample_chunk()).unwrap();
        bytes.push(0);
        assert!(matches!(
            decode_chunk(&bytes),
            Err(BytecodeFormatError::CorruptPayload(_))
        ));
    }

    #[test]
    fn huge_length_field_fails_cleanly() {
        let mut bytes = bytecode_header().to_vec();
        // `code` length of u64::MAX with nothing after it.
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        assert!(matches!(
            decode_chunk(&bytes),
            Err(BytecodeFormatError::CorruptPayload(_))
        ));
    }
}
