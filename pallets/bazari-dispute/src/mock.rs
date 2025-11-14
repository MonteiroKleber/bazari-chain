use crate as pallet_bazari_dispute;
use frame_support::{derive_impl, parameter_types, traits::{ConstU32, ConstU128, ConstU64}};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;
pub type AccountId = u64;
pub type Balance = u128;
pub type ItemId = u64;
pub type BlockNumber = u64;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Randomness: pallet_insecure_randomness_collective_flip,
		Uniques: pallet_uniques,
		Stores: pallet_stores,
		BazariIdentity: pallet_bazari_identity,
		BazariCommerce: pallet_bazari_commerce,
		BazariEscrow: pallet_bazari_escrow,
		BazariAffiliate: pallet_bazari_affiliate,
		BazariFulfillment: pallet_bazari_fulfillment,
		BazariDispute: pallet_bazari_dispute,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type AccountData = pallet_balances::AccountData<Balance>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = ();
	type FreezeIdentifier = ();
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<50>;
	type MaxFreezes = ();
	type RuntimeFreezeReason = ();
	type DoneSlashHandler = ();
}

impl pallet_insecure_randomness_collective_flip::Config for Test {}

impl pallet_uniques::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = ItemId;
	type ItemId = ItemId;
	type Currency = Balances;
	type CreateOrigin = frame_support::traits::AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	type Locker = ();
	type CollectionDeposit = ConstU128<0>;
	type ItemDeposit = ConstU128<0>;
	type MetadataDepositBase = ConstU128<0>;
	type AttributeDepositBase = ConstU128<0>;
	type DepositPerByte = ConstU128<0>;
	type StringLimit = ConstU32<128>;
	type KeyLimit = ConstU32<32>;
	type ValueLimit = ConstU32<64>;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

parameter_types! {
	pub const MaxCidLen: u32 = 100;
	pub const MaxOperators: u32 = 10;
	pub const CreationDeposit: Balance = 1000;
	pub const MaxStoresPerOwner: u32 = 100;
	pub const MaxItemsPerOrder: u32 = 50;
	pub const MaxItemNameLen: u32 = 100;
	pub const PlatformFeeBps: u32 = 250;
	pub const MaxHandleLen: u32 = 32;
	pub const MaxBadges: u32 = 10;
	pub const MaxBadgeCodeLen: u32 = 16;
	pub const MaxHandleHistory: u32 = 10;
	pub const HandleCooldownBlocks: u64 = 100;
	pub const MaxReasonCodeLen: u32 = 64;
	pub const MaxAuthorizedModules: u32 = 10;
}

impl pallet_stores::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type StoreId = ItemId;
	type MaxCidLen = MaxCidLen;
	type MaxOperators = MaxOperators;
	type CreationDeposit = CreationDeposit;
	type ReputationOrigin = frame_system::EnsureSigned<AccountId>;
	type MaxStoresPerOwner = MaxStoresPerOwner;
}

impl pallet_bazari_identity::Config for Test {
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

impl pallet_bazari_commerce::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type OrderId = u64;
	type MaxItemsPerOrder = MaxItemsPerOrder;
	type MaxItemNameLen = MaxItemNameLen;
	type PlatformFeeBps = PlatformFeeBps;
}

impl pallet_bazari_escrow::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type DAOOrigin = frame_system::EnsureRoot<AccountId>;
}

parameter_types! {
	pub const CommissionRates: [u32; 5] = [500, 250, 125, 62, 31];
	pub const MaxReferralDepth: u8 = 5;
	pub const MaxDirectReferrals: u32 = 1000;
}

impl pallet_bazari_affiliate::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type CommissionRates = CommissionRates;
	type MaxReferralDepth = MaxReferralDepth;
	type MaxDirectReferrals = MaxDirectReferrals;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinCourierStake: Balance = 1000;
	pub const MaxServiceAreas: u32 = 10;
	pub const MaxDeliveriesPerCourier: u32 = 100;
}

impl pallet_bazari_fulfillment::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MinCourierStake = MinCourierStake;
	type MaxServiceAreas = MaxServiceAreas;
	type MaxDeliveriesPerCourier = MaxDeliveriesPerCourier;
	type DAOOrigin = frame_system::EnsureRoot<AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	pub const CommitPhaseDuration: BlockNumber = 100; // 24h = ~14400 blocks in real chain
	pub const RevealPhaseDuration: BlockNumber = 100;
	pub const MinJurorReputation: u32 = 500;
}

impl pallet_bazari_dispute::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Randomness = Randomness;
	type CommitPhaseDuration = CommitPhaseDuration;
	type RevealPhaseDuration = RevealPhaseDuration;
	type MinJurorReputation = MinJurorReputation;
	type WeightInfo = ();
}

pub fn account(id: u64) -> AccountId {
	id
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default()
		.build_storage()
		.unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(account(1), 100_000),
			(account(2), 100_000),
			(account(3), 100_000),
			(account(4), 100_000),
			(account(5), 100_000),
			(account(6), 100_000),
			(account(7), 100_000),
			(account(8), 100_000),
			(account(9), 100_000),
			(account(10), 100_000),
		],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
