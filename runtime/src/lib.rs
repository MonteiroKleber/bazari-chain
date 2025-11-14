#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "512"]

#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod apis;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;
pub mod configs;

extern crate alloc;
use alloc::vec::Vec;
use sp_runtime::{
    generic, impl_opaque_keys,
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    MultiAddress, MultiSignature,
};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

pub mod genesis_config_presets;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;
    use sp_runtime::{
        generic,
        traits::{BlakeTwo256, Hash as HashT},
    };

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;
    /// Opaque block hash type.
    pub type Hash = <BlakeTwo256 as HashT>::Output;
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub aura: Aura,
        pub grandpa: Grandpa,
    }
}

// To learn more about runtime versioning, see:
// https://docs.substrate.io/main-docs/build/upgrade#runtime-versioning
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: alloc::borrow::Cow::Borrowed("bazari-runtime"),
    impl_name: alloc::borrow::Cow::Borrowed("bazari-runtime"),
    authoring_version: 1,
    // The version of the runtime specification. A full node will not attempt to use its native
    //   runtime in substitute for the on-chain Wasm runtime unless all of `spec_name`,
    //   `spec_version`, and `authoring_version` are the same between Wasm and native.
    // This value is set to 101 after renaming UNIT to BZR (breaking change)
    // FASE 3: Bumped to 102 after adding pallet-assets (storage layout change)
    // FASE 9: Bumped to 103 after adding pallet-vesting (storage layout change)
    // FASE 10: Bumped to 104 after fixing Treasury SpendOrigin (allow Council >= 50%)
    // FASE 11: Bumped to 105 after implementing Council origin support for Treasury.spendLocal
    // FASE 12: Bumped to 106 after fixing BadOrigin error by using sudo wrapper approach for Council Treasury spends
    // FASE 13: Bumped to 107 after fixing BadOrigin: Council can't call sudo, so using EitherOfDiverse to allow Council direct access
    // FASE 14: Bumped to 108 after fixing root cause: frontend threshold=1 + using idiomatic EitherOfDiverse pattern + lengthBound fix
    // FASE 15: Bumped to 109 after reducing SpendPeriod from 7 days to 30 minutes for pre-production testing (faster Treasury payouts)
    // FASE 16: Bumped to 110 after adjusting Democracy periods: LaunchPeriod 2h, VotingPeriod 1d, EnactmentPeriod 1h (was 7d/7d/2d)
    spec_version: 110,
    impl_version: 1,
    apis: apis::RUNTIME_API_VERSIONS,
    transaction_version: 1,
    system_version: 1,
};

mod block_times {
    /// This determines the average expected block time that we are targeting. Blocks will be
    /// produced at a minimum duration defined by `SLOT_DURATION`. `SLOT_DURATION` is picked up by
    /// `pallet_timestamp` which is in turn picked up by `pallet_aura` to implement `fn
    /// slot_duration()`.
    ///
    /// Change this to adjust the block time.
    pub const MILLI_SECS_PER_BLOCK: u64 = 6000;

    // NOTE: Currently it is not possible to change the slot duration after the chain has started.
    // Attempting to do so will brick block production.
    pub const SLOT_DURATION: u64 = MILLI_SECS_PER_BLOCK;
}
pub use block_times::*;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLI_SECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

pub const BLOCK_HASH_COUNT: BlockNumber = 2400;

/// BZR token constants
/// 1 BZR = 10^12 planck (12 decimals, like DOT/KSM)
pub const BZR: Balance = 1_000_000_000_000;
pub const MILLI_BZR: Balance = 1_000_000_000;     // 0.001 BZR
pub const MICRO_BZR: Balance = 1_000_000;         // 0.000001 BZR

/// Minimum balance to keep account alive (0.001 BZR)
pub const EXISTENTIAL_DEPOSIT: Balance = MILLI_BZR;

/// Governance constants
pub const PROPOSAL_BOND_PERCENT: u32 = 5;
pub const TREASURY_PROPOSAL_BOND_MIN: Balance = 100 * BZR;
pub const TREASURY_PROPOSAL_BOND_MAX: Balance = 500 * BZR;
/// Treasury spend period: 30 minutes (for quick testing/pre-production)
/// Production should use longer periods (e.g., 7 * DAYS)
pub const SPEND_PERIOD: BlockNumber = 30 * MINUTES; // 30 minutes = 300 blocks
pub const BOUNTY_DEPOSIT_BASE: Balance = BZR;
pub const BOUNTY_VALUE_MINIMUM: Balance = 10 * BZR;

/// Token metadata for RPC exposure
pub const TOKEN_SYMBOL: &str = "BZR";
pub const TOKEN_NAME: &str = "Bazari Token";
pub const TOKEN_DECIMALS: u8 = 12;

// Deprecated aliases for backwards compatibility
// TODO: Remove after 2-3 releases
#[deprecated(since = "0.2.0", note = "Use BZR instead")]
pub const UNIT: Balance = BZR;

#[deprecated(since = "0.2.0", note = "Use MILLI_BZR instead")]
pub const MILLI_UNIT: Balance = MILLI_BZR;

#[deprecated(since = "0.2.0", note = "Use MICRO_BZR instead")]
pub const MICRO_UNIT: Balance = MICRO_BZR;

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// An index to a block.
pub type BlockNumber = u32;

/// Uniques identifiers (pallet-uniques)
pub type CollectionId = u32;
pub type ItemId = u64;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The `TransactionExtension` to the basic transaction logic.
pub type TxExtension = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
    frame_system::WeightReclaim<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, TxExtension>;

/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, TxExtension>;

/// All migrations of the runtime, aside from the ones declared in the pallets.
///
/// This can be a tuple of types, each implementing `OnRuntimeUpgrade`.
#[allow(unused_parens)]
type Migrations = ();

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations,
>;

// Create the runtime by composing the FRAME pallets that were previously configured.
#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Runtime;

    #[runtime::pallet_index(0)]
    pub type System = frame_system;

    #[runtime::pallet_index(1)]
    pub type Timestamp = pallet_timestamp;

    #[runtime::pallet_index(2)]
    pub type Aura = pallet_aura;

    #[runtime::pallet_index(3)]
    pub type Grandpa = pallet_grandpa;

    #[runtime::pallet_index(4)]
    pub type Balances = pallet_balances;

    #[runtime::pallet_index(5)]
    pub type TransactionPayment = pallet_transaction_payment;

    #[runtime::pallet_index(6)]
    pub type Sudo = pallet_sudo;

    // Include the custom logic from the pallet-template in the runtime.
    #[runtime::pallet_index(7)]
    pub type Template = pallet_template;

    // Non-fungible assets (pallet-uniques) — added in Fase 1A
    #[runtime::pallet_index(8)]
    pub type Uniques = pallet_uniques;

    // Stores pallet (Fase 1B)
    #[runtime::pallet_index(9)]
    pub type Stores = pallet_stores;

    // Universal registry (Fase 1D) — opcional, sob feature flag
    #[cfg(feature = "with-universal-registry")]
    #[runtime::pallet_index(10)]
    pub type UniversalRegistry = pallet_universal_registry;

    // Bazari Identity pallet - Soulbound NFT profiles
    #[runtime::pallet_index(11)]
    pub type BazariIdentity = pallet_bazari_identity;

    // Fungible assets (FASE 3: ZARI Token)
    #[runtime::pallet_index(12)]
    pub type Assets = pallet_assets;

    // FASE 7: Governance Pallets
    #[runtime::pallet_index(13)]
    pub type Scheduler = pallet_scheduler;

    #[runtime::pallet_index(14)]
    pub type Preimage = pallet_preimage;

    #[runtime::pallet_index(15)]
    pub type Treasury = pallet_treasury;

    #[runtime::pallet_index(16)]
    pub type Multisig = pallet_multisig;

    #[runtime::pallet_index(17)]
    pub type Council = pallet_collective<Instance1>;

    #[runtime::pallet_index(18)]
    pub type TechnicalCommittee = pallet_collective<Instance2>;

    #[runtime::pallet_index(19)]
    pub type Democracy = pallet_democracy;

    // FASE 9: Vesting pallet for token release schedules
    #[runtime::pallet_index(20)]
    pub type Vesting = pallet_vesting;

    // Bazari Commerce pallet - Orders, Sales, Commissions
    #[runtime::pallet_index(21)]
    pub type BazariCommerce = pallet_bazari_commerce;

    // Bazari Escrow pallet - Secure fund locking for orders
    #[runtime::pallet_index(22)]
    pub type BazariEscrow = pallet_bazari_escrow;

    // Bazari Rewards pallet - Cashback and missions
    #[runtime::pallet_index(23)]
    pub type BazariRewards = pallet_bazari_rewards;

    // Bazari Attestation pallet - Cryptographic proofs for handoff and delivery
    #[runtime::pallet_index(24)]
    pub type BazariAttestation = pallet_bazari_attestation;

    // Bazari Fulfillment pallet - Courier registry, staking, and reputation
    #[runtime::pallet_index(25)]
    pub type BazariFulfillment = pallet_bazari_fulfillment;

    // Bazari Affiliate pallet - Multi-level affiliate commission system with DAG
    #[runtime::pallet_index(26)]
    pub type BazariAffiliate = pallet_bazari_affiliate;

    // Bazari Fee pallet - Auto-splitting de pagamentos (platform + affiliate + seller)
    #[runtime::pallet_index(27)]
    pub type BazariFee = pallet_bazari_fee;

    // Insecure Randomness Collective Flip - VRF source for dispute juror selection
    #[runtime::pallet_index(28)]
    pub type RandomnessCollectiveFlip = pallet_insecure_randomness_collective_flip;

    // Bazari Dispute pallet - VRF juror selection + commit-reveal voting
    #[runtime::pallet_index(29)]
    pub type BazariDispute = pallet_bazari_dispute;
}
