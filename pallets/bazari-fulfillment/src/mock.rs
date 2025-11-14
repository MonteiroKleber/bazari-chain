use crate as pallet_bazari_fulfillment;
use frame_support::{
	derive_impl, parameter_types,
};
use frame_system::EnsureRoot;
use sp_runtime::{
	traits::IdentityLookup,
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;
pub type AccountId = u64;
pub type Balance = u128;
pub type BlockNumber = u64;

// Configure mock runtime
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		BazariIdentity: pallet_bazari_identity,
		BazariFulfillment: pallet_bazari_fulfillment,
	}
);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type AccountData = pallet_balances::AccountData<Balance>;
}

// pallet-balances configuration
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
	type FreezeIdentifier = ();
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type MaxFreezes = ();
	type RuntimeFreezeReason = ();
	type DoneSlashHandler = ();
}

// pallet-bazari-identity configuration
parameter_types! {
	pub const MaxCidLen: u32 = 96;
	pub const MaxHandleLen: u32 = 32;
	pub const MaxBadges: u32 = 50;
	pub const MaxBadgeCodeLen: u32 = 32;
	pub const MaxHandleHistory: u32 = 10;
	pub const HandleCooldownBlocks: BlockNumber = 432000;
	pub const MaxReasonCodeLen: u32 = 64;
	pub const MaxAuthorizedModules: u32 = 10;
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

// pallet-bazari-fulfillment configuration
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
	type DAOOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
}

// Helper function to create test externalities
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	// Pre-fund accounts
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(1, 10_000), // Courier 1
			(2, 10_000), // Courier 2
			(3, 10_000), // Seller/Buyer
			(4, 500),    // Insufficient funds
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

// Helper function to create account
pub fn account(id: u64) -> AccountId {
	id
}
