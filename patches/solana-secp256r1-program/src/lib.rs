// Stub replacement for solana-secp256r1-program.
// Drops the openssl dependency entirely — safe for this wallet because we
// never call secp256r1 precompile verification at runtime.

use bytemuck::{Pod, Zeroable};
use solana_precompile_error::PrecompileError;
pub use solana_sdk_ids::secp256r1_program::{check_id, id, ID};

// ── Public structs expected by solana-precompiles ────────────────────────────

#[derive(Default, Debug, Copy, Clone, Zeroable, Pod, Eq, PartialEq)]
#[repr(C)]
pub struct Secp256r1SignatureOffsets {
    pub signature_offset: u16,
    pub signature_instruction_index: u16,
    pub public_key_offset: u16,
    pub public_key_instruction_index: u16,
    pub message_data_offset: u16,
    pub message_data_size: u16,
    pub message_instruction_index: u16,
}

pub const SIGNATURE_SERIALIZED_SIZE: usize = 64;
pub const COMPRESSED_PUBKEY_SERIALIZED_SIZE: usize = 33;
pub const DATA_START: usize = 1;
pub const SIGNATURE_OFFSETS_SERIALIZED_SIZE: usize =
    std::mem::size_of::<Secp256r1SignatureOffsets>();
pub const SIGNATURE_OFFSETS_START: usize = 2;

// ── verify — stub always returns InvalidInstructionDataSize ─────────────────
// solana-precompiles registers this function pointer but it is never called
// by our wallet code, which only issues SOL system transfers.

#[allow(deprecated)]
pub fn verify(
    _data: &[u8],
    _instruction_datas: &[&[u8]],
    _feature_set: &solana_feature_set::FeatureSet,
) -> Result<(), PrecompileError> {
    Err(PrecompileError::InvalidInstructionDataSize)
}
