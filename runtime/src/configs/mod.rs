// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org>

// Substrate and Polkadot dependencies
use frame_support::{
    derive_impl, parameter_types,
    traits::{
        AsEnsureOriginWithArg, ConstBool, ConstU128, ConstU32, ConstU64, ConstU8, VariantCountOf,
        EitherOfDiverse, MapSuccess,
    },
    weights::{
        constants::{RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND},
        IdentityFee, Weight,
    },
};
use frame_system::limits::{BlockLength, BlockWeights};
use frame_system::{EnsureRoot, EnsureSigned};
use pallet_transaction_payment::{ConstFeeMultiplier, FungibleAdapter, Multiplier};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::{traits::{One, AccountIdConversion, Replace}, Perbill, Permill};
use sp_version::RuntimeVersion;

// Local module imports
use super::{
    AccountId, Aura, Balance, Balances, Block, BlockNumber, CollectionId, Hash, ItemId, Nonce,
    OriginCaller, PalletInfo, Preimage, Runtime, RuntimeCall, RuntimeEvent, RuntimeFreezeReason,
    RuntimeHoldReason, RuntimeOrigin, RuntimeTask, Scheduler, System, EXISTENTIAL_DEPOSIT,
    MICRO_BZR, MILLI_BZR, SLOT_DURATION, VERSION,
};

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const Version: RuntimeVersion = VERSION;

    /// We allow for 2 seconds of compute with a 6 second average block time.
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::with_sensible_defaults(
        Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
        NORMAL_DISPATCH_RATIO,
    );
    pub RuntimeBlockLength: BlockLength = BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub const SS58Prefix: u8 = 42;
}

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`SoloChainDefaultConfig`](`struct@frame_system::config_preludes::SolochainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    /// The block type for the runtime.
    type Block = Block;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = RuntimeBlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = RuntimeBlockLength;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    /// Version of the runtime.
    type Version = Version;
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_aura::Config for Runtime {
    type AuthorityId = AuraId;
    type DisabledValidators = ();
    type MaxAuthorities = ConstU32<32>;
    type AllowMultipleBlocksPerSlot = ConstBool<false>;
    type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Runtime>;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    type WeightInfo = ();
    type MaxAuthorities = ConstU32<32>;
    type MaxNominators = ConstU32<0>;
    type MaxSetIdSessionEntries = ConstU64<0>;

    type KeyOwnerProof = sp_core::Void;
    type EquivocationReportSystem = ();
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Aura;
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type DoneSlashHandler = ();
}

parameter_types! {
    pub FeeMultiplier: Multiplier = Multiplier::one();
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = FungibleAdapter<Balances, ()>;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
    type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

/// Configure the pallet-template in pallets/template.
impl pallet_template::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_template::weights::SubstrateWeight<Runtime>;
}

// -------- pallet-uniques params already configured above --------

// -------- pallet-stores (Fase 1B) --------
parameter_types! {
    pub const StoresMaxCidLen: u32 = 96; // CID v1 (bytes) safe cap
    pub const StoresMaxOperators: u32 = 5;
    pub const StoresCreationDeposit: Balance = 0;
    pub const StoresMaxStoresPerOwner: u32 = 64;
}

#[cfg(feature = "with-universal-registry")]
parameter_types! {
    pub const RegistryMaxNamespaceLen: u32 = 48;
    pub const RegistryMaxHeadCidLen: u32 = 96;
}

impl pallet_stores::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type StoreId = ItemId;
    type MaxCidLen = StoresMaxCidLen;
    type MaxOperators = StoresMaxOperators;
    type CreationDeposit = StoresCreationDeposit;
    type ReputationOrigin = EnsureSigned<AccountId>;
    type MaxStoresPerOwner = StoresMaxStoresPerOwner;
    #[cfg(feature = "with-universal-registry")]
    type Registry = ();
}

#[cfg(feature = "with-universal-registry")]
impl pallet_universal_registry::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxNamespaceLen = RegistryMaxNamespaceLen;
    type MaxHeadCidLen = RegistryMaxHeadCidLen;
    type SetHeadOrigin = EnsureSigned<AccountId>;
}

// --- pallet-uniques (Fase 1A) ---
parameter_types! {
    // Depósitos moderados para evitar DoS por armazenamento
    pub const UniquesCollectionDeposit: Balance = 10 * MILLI_BZR; // depósito para coleção
    pub const UniquesItemDeposit: Balance = 1 * MILLI_BZR;        // depósito por item
    pub const UniquesKeyLimit: u32 = 32;                           // limite para chaves de atributos
    pub const UniquesValueLimit: u32 = 256;                        // limite para valores de atributos
    pub const UniquesStringLimit: u32 = 256;                       // limite para strings de metadata
    pub const UniquesMetadataDepositBase: Balance = 1 * MILLI_BZR;    // base para metadata
    pub const UniquesAttributeDepositBase: Balance = 1 * MILLI_BZR;   // base para atributo
    pub const UniquesDepositPerByte: Balance = MICRO_BZR;             // custo por byte armazenado
}

impl pallet_uniques::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type Currency = Balances;
    // Root pode forçar operações administrativas
    type ForceOrigin = EnsureRoot<AccountId>;
    // Criação de coleção por qualquer conta assinada (parametrizável futuramente)
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
    type Locker = ();
    type CollectionDeposit = UniquesCollectionDeposit;
    type ItemDeposit = UniquesItemDeposit;
    type KeyLimit = UniquesKeyLimit;
    type ValueLimit = UniquesValueLimit;
    type StringLimit = UniquesStringLimit;
    type MetadataDepositBase = UniquesMetadataDepositBase;
    type AttributeDepositBase = UniquesAttributeDepositBase;
    type DepositPerByte = UniquesDepositPerByte;
    type WeightInfo = pallet_uniques::weights::SubstrateWeight<Runtime>;
}

// --- pallet-bazari-identity (Sprint 1-2) ---
parameter_types! {
    pub const MaxCidLen: u32 = 96;
    pub const MaxHandleLen: u32 = 32;
    pub const MaxBadges: u32 = 50;
    pub const MaxBadgeCodeLen: u32 = 32;
    pub const MaxHandleHistory: u32 = 10;
    pub const HandleCooldownBlocks: BlockNumber = 432000; // ~30 dias (6s por bloco)
    pub const MaxReasonCodeLen: u32 = 64;
    pub const MaxAuthorizedModules: u32 = 100;
}

impl pallet_bazari_identity::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxCidLen = MaxCidLen;
    type MaxHandleLen = MaxHandleLen;
    type MaxBadges = MaxBadges;
    type MaxBadgeCodeLen = MaxBadgeCodeLen;
    type MaxHandleHistory = MaxHandleHistory;
    type HandleCooldownBlocks = HandleCooldownBlocks;
    type MaxReasonCodeLen = MaxReasonCodeLen;
    type MaxAuthorizedModules = MaxAuthorizedModules;
}

// --- pallet-bazari-commerce (Orders, Sales, Commissions) ---
parameter_types! {
    pub const MaxItemsPerOrder: u32 = 50;
    pub const MaxItemNameLen: u32 = 128;
    pub const PlatformFeeBps: u32 = 250; // 2.5% = 250 basis points
}

impl pallet_bazari_commerce::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type OrderId = u64;
    type MaxItemsPerOrder = MaxItemsPerOrder;
    type MaxItemNameLen = MaxItemNameLen;
    type PlatformFeeBps = PlatformFeeBps;
}

// --- pallet-bazari-escrow (Escrow for order payments) ---
impl pallet_bazari_escrow::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    // DAOOrigin: Either Root OR Council (≥50% majority) can force refunds
    type DAOOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionAtLeast<AccountId, pallet_collective::Instance1, 1, 2>
    >;
}

// --- pallet-assets (FASE 3: ZARI Token) ---
parameter_types! {
    // Depósito para criar um asset (10 BZR para evitar spam)
    pub const AssetDeposit: Balance = 10 * crate::BZR;

    // Depósito por conta que possui o asset (0.1 BZR - storage mínimo)
    pub const AssetAccountDeposit: Balance = 100 * crate::MILLI_BZR;

    // Depósito base para metadata (1 BZR)
    pub const MetadataDepositBase: Balance = 1 * crate::BZR;

    // Depósito por byte de metadata (0.001 BZR por byte)
    pub const MetadataDepositPerByte: Balance = 1 * crate::MILLI_BZR;

    // Depósito para aprovações (delegações) - 0.1 BZR
    pub const ApprovalDeposit: Balance = 100 * crate::MILLI_BZR;

    // Limite de caracteres para strings (nome/símbolo)
    pub const StringLimit: u32 = 50;

    // Limite de items removíveis por chamada (anti-spam)
    pub const RemoveItemsLimit: u32 = 1000;
}

impl pallet_assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    // Tipo de balance (mesma que Balances - u128)
    type Balance = Balance;

    // AssetId como u32 (permite até ~4 bilhões de assets)
    type AssetId = u32;
    type AssetIdParameter = codec::Compact<u32>;

    // BZR usado para pagar depósitos de storage
    type Currency = Balances;

    // Qualquer conta pode criar asset (em produção, poderia ser RestrictedOrigin)
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;

    // Root pode forçar operações (freeze, thaw, destroy)
    type ForceOrigin = EnsureRoot<AccountId>;

    // Depósitos configurados acima
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;

    // Limites de string
    type StringLimit = StringLimit;

    // Sem freezer customizado (usa padrão)
    type Freezer = ();

    // Sem data extra por asset
    type Extra = ();

    // Weights padrão do Substrate
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;

    // Limite anti-DoS para remoção em lote
    type RemoveItemsLimit = RemoveItemsLimit;

    // Sem callback customizado
    type CallbackHandle = ();

    // Holder type (para contas que seguram assets)
    type Holder = ();

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

// --- pallet-bazari-rewards (Cashback and Missions) ---
parameter_types! {
    pub const ZariAssetId: u32 = 1; // ZARI token = AssetId 1
}

impl pallet_bazari_rewards::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Assets = pallet_assets::Pallet<Runtime>;
    type ZariAssetId = ZariAssetId;
    // DAO or Council can create missions
    type DAOOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionAtLeast<AccountId, pallet_collective::Instance1, 1, 2>
    >;
    type WeightInfo = ();
}

// --- pallet-bazari-attestation (Proof of Handoff & Delivery) ---
parameter_types! {
    pub const MaxSigners: u32 = 10;
    pub const MaxCidLength: u32 = 64;
}

impl pallet_bazari_attestation::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxSigners = MaxSigners;
    type MaxCidLength = MaxCidLength;
    type WeightInfo = ();
}

// --- pallet-bazari-fulfillment (Courier Registry, Staking & Reputation) ---
parameter_types! {
    pub const MinCourierStake: Balance = 1000 * crate::BZR;
    pub const MaxServiceAreas: u32 = 10;
    pub const MaxDeliveriesPerCourier: u32 = 100;
}

impl pallet_bazari_fulfillment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MinCourierStake = MinCourierStake;
    type MaxServiceAreas = MaxServiceAreas;
    type MaxDeliveriesPerCourier = MaxDeliveriesPerCourier;
    // DAOOrigin: Either Root OR Council (≥50% majority) can slash couriers
    type DAOOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionAtLeast<AccountId, pallet_collective::Instance1, 1, 2>
    >;
    type WeightInfo = ();
}

// pallet-bazari-affiliate configuration
parameter_types! {
	// Commission rates per level in basis points (5%, 2.5%, 1.25%, 0.62%, 0.31%)
	pub const CommissionRates: [u32; 5] = [500, 250, 125, 62, 31];
	// Maximum referral depth (5 levels)
	pub const MaxReferralDepth: u8 = 5;
	// Maximum direct referrals per account
	pub const MaxDirectReferrals: u32 = 1000;
}

impl pallet_bazari_affiliate::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type CommissionRates = CommissionRates;
	type MaxReferralDepth = MaxReferralDepth;
	type MaxDirectReferrals = MaxDirectReferrals;
	type WeightInfo = ();
}

// pallet-bazari-fee configuration
parameter_types! {
	// Default platform fee (5% = 500 bps)
	pub const DefaultPlatformFee: u32 = 500;
	// Treasury account (derived from PalletId)
	pub TreasuryAccountId: AccountId = TreasuryPalletId::get().into_account_truncating();
	// Minimum order amount to apply fees
	pub const MinOrderAmount: Balance = 100 * crate::MILLI_BZR; // 0.1 BZR
}

impl pallet_bazari_fee::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type DefaultPlatformFee = DefaultPlatformFee;
	type TreasuryAccount = TreasuryAccountId;
	type MinOrderAmount = MinOrderAmount;
	// DAOOrigin: Either Root OR Council (≥50% majority) can update platform fee
	type DAOOrigin = EitherOfDiverse<
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<AccountId, pallet_collective::Instance1, 1, 2>
	>;
	type WeightInfo = ();
}

// --- FASE 7: Governance Pallets ---

// --- pallet-preimage (required by scheduler & democracy) ---
parameter_types! {
    pub const PreimageBaseDeposit: Balance = 1 * crate::BZR;
    pub const PreimageByteDeposit: Balance = 10 * crate::MICRO_BZR;
    pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type Consideration = ();
}

// --- pallet-scheduler (required by democracy, treasury, etc) ---
parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
        RuntimeBlockWeights::get().max_block;
}

impl pallet_scheduler::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = ConstU32<50>;
    type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
    type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
    type Preimages = Preimage;
    type BlockNumberProvider = System;
}

// --- pallet-treasury (community fund management) ---
parameter_types! {
    pub const TreasuryPalletId: frame_support::PalletId = frame_support::PalletId(*b"py/trsry");
    pub const SpendPeriod: BlockNumber = crate::SPEND_PERIOD;
    pub const Burn: Permill = Permill::from_percent(0); // No burn (0%)
    pub const MaxApprovals: u32 = 100;
    pub const PayoutSpendPeriod: BlockNumber = 30 * crate::DAYS;
    pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
    // Max spend limit for both Root and Council origins
    pub const CouncilSpendMax: Balance = 1_000_000 * crate::BZR;
}

impl pallet_treasury::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = TreasuryPalletId;
    type Currency = Balances;
    type RejectOrigin = EnsureRoot<AccountId>;
    // SpendOrigin: Either Root OR Council (≥50% majority) can spend from treasury
    // Max spend of 1M BZR per call for both origins
    // Council executes treasury.spendLocal() directly via motions
    // Root can also execute for emergency/testing purposes
    // Uses idiomatic EitherOfDiverse pattern from Polkadot/Kusama
    type SpendOrigin = MapSuccess<
        EitherOfDiverse<
            frame_system::EnsureRootWithSuccess<AccountId, CouncilSpendMax>,
            pallet_collective::EnsureProportionAtLeast<AccountId, pallet_collective::Instance1, 1, 2>
        >,
        Replace<CouncilSpendMax>
    >;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type BurnDestination = ();
    type MaxApprovals = MaxApprovals;
    type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
    type SpendFunds = ();
    type AssetKind = ();
    type Beneficiary = AccountId;
    type BeneficiaryLookup = sp_runtime::traits::IdentityLookup<AccountId>;
    type Paymaster = frame_support::traits::tokens::PayFromAccount<Balances, TreasuryAccount>;
    type BalanceConverter = frame_support::traits::tokens::UnityAssetBalanceConversion;
    type PayoutPeriod = PayoutSpendPeriod;
    type BlockNumberProvider = System;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

// --- pallet-multisig (multi-signature accounts) ---
parameter_types! {
    pub const MultisigDepositBase: Balance = 100 * crate::MILLI_BZR;
    pub const MultisigDepositFactor: Balance = 50 * crate::MILLI_BZR;
    pub const MaxSignatories: u32 = 20;
}

impl pallet_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = MultisigDepositBase;
    type DepositFactor = MultisigDepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
    type BlockNumberProvider = System;
}

// --- pallet-vesting (token vesting schedules) ---
parameter_types! {
    /// Minimum amount for vested transfer (100 BZR)
    pub const MinVestedTransfer: Balance = 100 * crate::BZR;

    /// Withdraw reasons for unvested funds
    /// Allow all except TRANSFER and RESERVE
    pub UnvestedFundsAllowedWithdrawReasons: frame_support::traits::WithdrawReasons =
        frame_support::traits::WithdrawReasons::except(
            frame_support::traits::WithdrawReasons::TRANSFER |
            frame_support::traits::WithdrawReasons::RESERVE
        );

    /// Maximum number of vesting schedules per account
    pub const MaxVestingSchedules: u32 = 28;
}

impl pallet_vesting::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BlockNumberToBalance = sp_runtime::traits::ConvertInto;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = pallet_vesting::weights::SubstrateWeight<Runtime>;
    type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
    type BlockNumberProvider = System;

    // Maximum vesting schedules constant
    const MAX_VESTING_SCHEDULES: u32 = 28;
}

// --- pallet-collective: Council ---
parameter_types! {
    pub const CouncilMotionDuration: BlockNumber = 7 * crate::DAYS;
    pub const CouncilMaxProposals: u32 = 100;
    pub const CouncilMaxMembers: u32 = 13;
    // Allow proposals to use up to 50% of block weight
    // NORMAL_DISPATCH_RATIO is 75%, we use 50% to be safe
    pub MaxCollectivesProposalWeight: Weight = Weight::from_parts(
        WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2) / 2,
        u64::MAX,
    );
}

impl pallet_collective::Config<pallet_collective::Instance1> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = EnsureRoot<AccountId>;
    // Increase MaxProposalWeight to allow treasury.spendLocal proposals
    // Using MAXIMUM_BLOCK_WEIGHT / 2 (50% of block weight)
    type MaxProposalWeight = MaxCollectivesProposalWeight;
    type DisapproveOrigin = EnsureRoot<AccountId>;
    type KillOrigin = EnsureRoot<AccountId>;
    type Consideration = ();
}

// --- pallet-collective: TechnicalCommittee ---
parameter_types! {
    pub const TechnicalMotionDuration: BlockNumber = 7 * crate::DAYS;
    pub const TechnicalMaxProposals: u32 = 100;
    pub const TechnicalMaxMembers: u32 = 7;
}

pub type TechnicalCommitteeInstance = pallet_collective::Instance2;

impl pallet_collective::Config<TechnicalCommitteeInstance> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = TechnicalMotionDuration;
    type MaxProposals = TechnicalMaxProposals;
    type MaxMembers = TechnicalMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = EnsureRoot<AccountId>;
    type MaxProposalWeight = MaxCollectivesProposalWeight;
    type DisapproveOrigin = EnsureRoot<AccountId>;
    type KillOrigin = EnsureRoot<AccountId>;
    type Consideration = ();
}

// --- pallet-democracy (on-chain voting) ---
parameter_types! {
    // Democracy periods adjusted for pre-production/testing
    // LaunchPeriod: How often the most-endorsed proposal becomes a referendum
    pub const LaunchPeriod: BlockNumber = 2 * crate::HOURS; // Was: 7 * DAYS
    // VotingPeriod: How long referendum voting lasts
    pub const VotingPeriod: BlockNumber = 1 * crate::DAYS;  // Was: 7 * DAYS
    // FastTrackVotingPeriod: For emergency/fast-tracked proposals
    pub const FastTrackVotingPeriod: BlockNumber = 3 * crate::HOURS;
    // MinimumDeposit: Required to propose (and to second)
    pub const MinimumDeposit: Balance = 100 * crate::BZR;
    // EnactmentPeriod: Delay between approval and execution
    pub const EnactmentPeriod: BlockNumber = 1 * crate::HOURS; // Was: 2 * DAYS
    // CooloffPeriod: Delay before rejected proposal can be re-submitted
    pub const CooloffPeriod: BlockNumber = 7 * crate::DAYS;
    pub const MaxVotes: u32 = 100;
    pub const MaxProposals: u32 = 100;
    pub const MaxDeposits: u32 = 100;
    pub const MaxBlacklisted: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EnactmentPeriod = EnactmentPeriod;
    type LaunchPeriod = LaunchPeriod;
    type VotingPeriod = VotingPeriod;
    type VoteLockingPeriod = EnactmentPeriod;
    type MinimumDeposit = MinimumDeposit;
    type ExternalOrigin = EnsureRoot<AccountId>;
    type ExternalMajorityOrigin = EnsureRoot<AccountId>;
    type ExternalDefaultOrigin = EnsureRoot<AccountId>;
    type SubmitOrigin = EnsureSigned<AccountId>;
    type FastTrackOrigin = EnsureRoot<AccountId>;
    type InstantOrigin = EnsureRoot<AccountId>;
    type InstantAllowed = ConstBool<true>;
    type FastTrackVotingPeriod = FastTrackVotingPeriod;
    type CancellationOrigin = EnsureRoot<AccountId>;
    type BlacklistOrigin = EnsureRoot<AccountId>;
    type CancelProposalOrigin = EnsureRoot<AccountId>;
    type VetoOrigin = frame_system::EnsureNever<AccountId>;
    type CooloffPeriod = CooloffPeriod;
    type Slash = ();
    type Scheduler = Scheduler;
    type PalletsOrigin = OriginCaller;
    type MaxVotes = MaxVotes;
    type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
    type MaxProposals = MaxProposals;
    type Preimages = Preimage;
    type MaxDeposits = MaxDeposits;
    type MaxBlacklisted = MaxBlacklisted;
}


// ===== Randomness Collective Flip =====
impl pallet_insecure_randomness_collective_flip::Config for Runtime {}

// ===== Bazari Dispute =====
parameter_types! {
	pub const CommitPhaseDuration: BlockNumber = 14_400; // ~24h (6s blocks)
	pub const RevealPhaseDuration: BlockNumber = 14_400; // ~24h (6s blocks)
	pub const MinJurorReputation: u32 = 500;
}

impl pallet_bazari_dispute::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Randomness = crate::RandomnessCollectiveFlip;
	type CommitPhaseDuration = CommitPhaseDuration;
	type RevealPhaseDuration = RevealPhaseDuration;
	type MinJurorReputation = MinJurorReputation;
	type WeightInfo = ();
}

// Bazari Work Agreements - Registro on-chain de acordos de trabalho
impl pallet_bazari_work_agreements::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

// --- Bazari Recurring Payments (Bazari Pay PROMPT-04) ---
parameter_types! {
	/// Maximum contracts per account
	pub const RecurringPaymentsMaxContractsPerAccount: u32 = 100;
}

impl pallet_bazari_recurring_payments::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MaxContractsPerAccount = RecurringPaymentsMaxContractsPerAccount;
}

