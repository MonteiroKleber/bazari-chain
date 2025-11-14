use crate as pallet_bazari_rewards;
use frame_support::{
	derive_impl, parameter_types,
	traits::{AsEnsureOriginWithArg, ConstU32},
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
pub type AssetId = u32;

// Configure a mock runtime to test the pallet
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Assets: pallet_assets,
		BazariRewards: pallet_bazari_rewards,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
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

parameter_types! {
	pub const AssetDeposit: Balance = 10;
	pub const AssetAccountDeposit: Balance = 1;
	pub const ApprovalDeposit: Balance = 1;
	pub const StringLimit: u32 = 50;
	pub const MetadataDepositBase: Balance = 1;
	pub const MetadataDepositPerByte: Balance = 1;
}

impl pallet_assets::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type AssetId = AssetId;
	type AssetIdParameter = codec::Compact<AssetId>;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	type AssetDeposit = AssetDeposit;
	type AssetAccountDeposit = AssetAccountDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type ApprovalDeposit = ApprovalDeposit;
	type StringLimit = StringLimit;
	type Freezer = ();
	type Extra = ();
	type CallbackHandle = ();
	type WeightInfo = ();
	type RemoveItemsLimit = ConstU32<1000>;
	type Holder = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
	pub const ZariAssetId: AssetId = 1;
}

impl pallet_bazari_rewards::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Assets = Assets;
	type ZariAssetId = ZariAssetId;
	type DAOOrigin = frame_system::EnsureRoot<AccountId>;
	type WeightInfo = ();
}

// Build genesis storage according to the mock runtime
pub fn new_test_ext() -> TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(1, 1_000_000_000_000_000), // Buyer with 1M BZR
			(2, 1_000_000_000_000_000), // Another user
			(99, 1_000_000_000_000_000), // Admin account
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	// Initialize ZARI asset (AssetId 1)
	pallet_assets::GenesisConfig::<Test> {
		assets: vec![
			// AssetId 1 = ZARI, owner = account 99, is_sufficient = true, min_balance = 1
			(1, 99, true, 1),
		],
		metadata: vec![
			// AssetId 1, name, symbol, decimals
			(1, b"Bazari Governance Token".to_vec(), b"ZARI".to_vec(), 12),
		],
		accounts: vec![
			// (AssetId, AccountId, Balance)
			(1, 99, 21_000_000_000_000_000_000), // 21M ZARI to admin (with 12 decimals)
		],
		next_asset_id: Some(2), // Next available asset ID
	}
	.assimilate_storage(&mut t)
	.unwrap();

	// Initialize cashback rates
	pallet_bazari_rewards::GenesisConfig::<Test> {
		cashback_tiers: vec![
			pallet_bazari_rewards::CashbackTier {
				threshold: 0, // 0-99 BZR
				percentage: 1,
			},
			pallet_bazari_rewards::CashbackTier {
				threshold: 100_000_000_000_000, // 100-499 BZR
				percentage: 2,
			},
			pallet_bazari_rewards::CashbackTier {
				threshold: 500_000_000_000_000, // 500+ BZR
				percentage: 3,
			},
		],
		_phantom: Default::default(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
	});
	ext
}
