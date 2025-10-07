use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::BoundedVec;
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

/// Tipo para representar módulos autorizados
pub type ModuleId = u32;

/// Tipo para identificar perfis
pub type ProfileId = u64;

/// Badge conquistado por um perfil
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxCodeLen))]
pub struct Badge<MaxCodeLen: Get<u32>> {
    /// Código identificador do badge (ex: "early_adopter", "top_seller")
    pub code: BoundedVec<u8, MaxCodeLen>,
    /// ID do módulo que emitiu o badge
    pub issuer: ModuleId,
    /// Timestamp de quando foi emitido (BlockNumber)
    pub issued_at: u64,
    /// Timestamp de revogação (None se ativo)
    pub revoked_at: Option<u64>,
}

/// Registro histórico de mudança de handle
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxHandleLen))]
pub struct HandleRecord<MaxHandleLen: Get<u32>> {
    /// O handle anterior
    pub handle: BoundedVec<u8, MaxHandleLen>,
    /// Número do bloco onde ocorreu a mudança
    pub changed_at: u64,
}

// Precisamos do trait Get
use frame_support::traits::Get;
