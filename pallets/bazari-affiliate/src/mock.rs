use crate as pallet_bazari_affiliate;
use frame_support::{derive_impl, parameter_types, traits::{ConstU32, ConstU128}};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;
pub type AccountId = u64;
pub type Balance = u128;
pub type ItemId = u64;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Uniques: pallet_uniques,
		Stores: pallet_stores,
		BazariCommerce: pallet_bazari_commerce,
		BazariAffiliate: pallet_bazari_affiliate,
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

parameter_types! {
	pub const MaxItemsPerOrder: u32 = 50;
	pub const MaxItemNameLen: u32 = 100;
	pub const PlatformFeeBps: u32 = 250;
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
	pub const CommissionRates: [u32; 5] = [500, 250, 125, 62, 31]; // 5%, 2.5%, 1.25%, 0.62%, 0.31%
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

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(1, 10_000),
			(2, 10_000),
			(3, 10_000),
			(4, 10_000),
			(5, 10_000),
			(6, 10_000),
		],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
	});
	ext
}

pub fn account(id: u64) -> AccountId {
	id
}
