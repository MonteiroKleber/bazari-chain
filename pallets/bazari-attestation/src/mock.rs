use crate as pallet_bazari_attestation;
use frame_support::{
	derive_impl, parameter_types,
	traits::{AsEnsureOriginWithArg, ConstU32, ConstU64},
};
use frame_system::EnsureSigned;
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;
pub type AccountId = u64;
pub type Balance = u128;
pub type BlockNumber = u64;
pub type CollectionId = u32;
pub type ItemId = u64;

// Configure mock runtime
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Uniques: pallet_uniques,
		Stores: pallet_stores,
		BazariCommerce: pallet_bazari_commerce,
		BazariAttestation: pallet_bazari_attestation,
	}
);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
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
	type RuntimeFreezeReason = ();
	type FreezeIdentifier = ();
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type MaxFreezes = ();
	type DoneSlashHandler = ();
}

// pallet-uniques configuration
parameter_types! {
	pub const CollectionDeposit: Balance = 10;
	pub const ItemDeposit: Balance = 1;
	pub const MetadataDepositBase: Balance = 1;
	pub const AttributeDepositBase: Balance = 1;
	pub const DepositPerByte: Balance = 1;
	pub const StringLimit: u32 = 128;
	pub const KeyLimit: u32 = 32;
	pub const ValueLimit: u32 = 256;
}

impl pallet_uniques::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = CollectionId;
	type ItemId = ItemId;
	type Currency = Balances;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type Locker = ();
	type CollectionDeposit = CollectionDeposit;
	type ItemDeposit = ItemDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = StringLimit;
	type KeyLimit = KeyLimit;
	type ValueLimit = ValueLimit;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

// pallet-stores configuration
parameter_types! {
	pub const MaxCidLen: u32 = 64;
	pub const MaxOperators: u32 = 10;
	pub const CreationDeposit: Balance = 100;
	pub const MaxStoresPerOwner: u32 = 100;
}

// Custom origin that returns AccountId (for tests, always returns account 99)
pub struct TestReputationOrigin;
impl frame_support::traits::EnsureOrigin<RuntimeOrigin> for TestReputationOrigin {
	type Success = AccountId;
	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		match o.clone().into() {
			Ok(frame_system::RawOrigin::Root) => Ok(99), // Root becomes account 99
			Ok(frame_system::RawOrigin::Signed(who)) => Ok(who),
			_ => Err(o),
		}
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		Ok(RuntimeOrigin::root())
	}
}

impl pallet_stores::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type StoreId = u64;
	type MaxCidLen = MaxCidLen;
	type MaxOperators = MaxOperators;
	type CreationDeposit = CreationDeposit;
	type ReputationOrigin = TestReputationOrigin;
	type MaxStoresPerOwner = MaxStoresPerOwner;
}

parameter_types! {
	pub const MaxItemsPerOrder: u32 = 50;
	pub const MaxItemNameLen: u32 = 100;
	pub const PlatformFeeBps: u32 = 250; // 2.5%
}

impl pallet_bazari_commerce::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type OrderId = u64;
	type MaxItemsPerOrder = MaxItemsPerOrder;
	type MaxItemNameLen = MaxItemNameLen;
	type PlatformFeeBps = PlatformFeeBps;
}

parameter_types! {
	pub const MaxSigners: u32 = 10;
	pub const MaxCidLength: u32 = 64;
}

impl pallet_bazari_attestation::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxSigners = MaxSigners;
	type MaxCidLength = MaxCidLength;
	type WeightInfo = ();
}

// Build genesis storage
pub fn new_test_ext() -> TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default()
		.build_storage()
		.unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(1, 1_000_000_000_000_000), // Seller
			(2, 1_000_000_000_000_000), // Courier
			(3, 1_000_000_000_000_000), // Buyer
			(4, 1_000_000_000_000_000), // Witness
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
	});
	ext
}

// Helper function to create account
pub fn account(id: u64) -> AccountId {
	id
}
