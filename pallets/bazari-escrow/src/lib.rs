#![cfg_attr(not(feature = "std"), no_std)]

//! # Bazari Escrow Pallet
//!
//! This pallet provides secure on-chain escrow for payments with reserve/unreserve pattern.
//!
//! ## Overview
//!
//! The Escrow pallet provides:
//! - Lock funds in escrow using Currency::reserve()
//! - Release funds to beneficiary (seller) after delivery
//! - Refund to depositor (buyer) if cancelled
//! - Partial refund for dispute resolution
//!
//! ## Extrinsics
//!
//! - `lock_funds` - Reserve funds from buyer's balance
//! - `release_funds` - Transfer reserved funds to seller
//! - `refund` - Return reserved funds to buyer
//! - `partial_refund` - Split funds between buyer and seller

extern crate alloc;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use alloc::vec::Vec;
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, ExistenceRequirement, ReservableCurrency},
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_runtime::{
		traits::{CheckedAdd, One, Saturating, Zero},
	};

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Escrow status state machine
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum EscrowStatus {
		/// Funds locked, awaiting release or refund
		Locked,
		/// Funds released to beneficiary
		Released,
		/// Funds refunded to depositor
		Refunded,
		/// Partially released (split between parties)
		PartialRefund,
		/// Under dispute
		Disputed,
	}

	/// Complete escrow record
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug))]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct Escrow<T: Config> {
		pub order_id: u64,
		pub buyer: T::AccountId,
		pub seller: T::AccountId,
		pub amount_locked: BalanceOf<T>,
		pub amount_released: BalanceOf<T>,
		pub status: EscrowStatus,
		pub locked_at: BlockNumberFor<T>,
		pub updated_at: BlockNumberFor<T>,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Currency type for handling balances
		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		/// Origin that can force refunds (DAO/Council)
		type DAOOrigin: EnsureOrigin<Self::RuntimeOrigin>;
	}

	// ============================================
	// STORAGE
	// ============================================

	/// Escrows storage map: OrderId => Escrow
	#[pallet::storage]
	#[pallet::getter(fn escrows)]
	pub type Escrows<T: Config> = StorageMap<_, Blake2_128Concat, u64, Escrow<T>, OptionQuery>;

	// ============================================
	// EVENTS
	// ============================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Funds locked in escrow [order_id, buyer, amount]
		FundsLocked {
			order_id: u64,
			buyer: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// Funds released to seller [order_id, seller, amount]
		FundsReleased {
			order_id: u64,
			seller: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// Funds refunded to buyer [order_id, buyer, amount]
		Refunded {
			order_id: u64,
			buyer: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// Partial refund executed [order_id, buyer_amount, seller_amount]
		PartialRefund {
			order_id: u64,
			buyer_amount: BalanceOf<T>,
			seller_amount: BalanceOf<T>,
		},
		/// Escrow marked as disputed (FASE 6 - NEW) [order_id]
		EscrowDisputed {
			order_id: u64,
		},
	}

	// ============================================
	// ERRORS
	// ============================================

	#[pallet::error]
	pub enum Error<T> {
		/// Order not found in commerce pallet
		OrderNotFound,
		/// Escrow not found
		EscrowNotFound,
		/// Caller is not authorized
		Unauthorized,
		/// Invalid escrow status for this operation
		InvalidStatus,
		/// Amount mismatch
		AmountMismatch,
		/// Insufficient balance
		InsufficientBalance,
		/// Escrow already exists for this order
		EscrowAlreadyExists,
	}

	// ============================================
	// EXTRINSICS
	// ============================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Lock funds in escrow
		///
		/// - `order_id`: Order ID from commerce pallet
		/// - `seller`: Seller account who will receive funds
		/// - `amount`: Amount to lock
		///
		/// Reserves funds from buyer's balance.
		/// Emits `FundsLocked` event.
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn lock_funds(
			origin: OriginFor<T>,
			order_id: u64,
			seller: T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			// Ensure escrow doesn't already exist
			ensure!(
				!Escrows::<T>::contains_key(order_id),
				Error::<T>::EscrowAlreadyExists
			);

			// Reserve funds from buyer
			<T as Config>::Currency::reserve(&buyer, amount).map_err(|_| Error::<T>::InsufficientBalance)?;

			let block_number = frame_system::Pallet::<T>::block_number();

			// Create escrow
			let escrow = Escrow {
				order_id,
				buyer: buyer.clone(),
				seller: seller.clone(),
				amount_locked: amount,
				amount_released: Zero::zero(),
				status: EscrowStatus::Locked,
				locked_at: block_number,
				updated_at: block_number,
			};

			Escrows::<T>::insert(order_id, escrow);

			Self::deposit_event(Event::FundsLocked {
				order_id,
				buyer,
				amount,
			});

			Ok(())
		}

		/// Release funds to seller
		///
		/// - `order_id`: Order ID
		///
		/// Can be called by buyer (confirm delivery) or automatically.
		/// Transfers reserved funds to seller.
		/// Emits `FundsReleased` event.
		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn release_funds(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
			let caller = ensure_signed(origin)?;

			let mut escrow = Escrows::<T>::get(order_id).ok_or(Error::<T>::EscrowNotFound)?;

			// Validate caller is buyer
			ensure!(caller == escrow.buyer, Error::<T>::Unauthorized);

			// Validate status
			ensure!(
				escrow.status == EscrowStatus::Locked,
				Error::<T>::InvalidStatus
			);

			// Unreserve from buyer
			<T as Config>::Currency::unreserve(&escrow.buyer, escrow.amount_locked);

			// Transfer to seller
			<T as Config>::Currency::transfer(
				&escrow.buyer,
				&escrow.seller,
				escrow.amount_locked,
				ExistenceRequirement::KeepAlive,
			)?;

			// Update escrow
			escrow.amount_released = escrow.amount_locked;
			escrow.status = EscrowStatus::Released;
			escrow.updated_at = frame_system::Pallet::<T>::block_number();

			Escrows::<T>::insert(order_id, escrow.clone());

			Self::deposit_event(Event::FundsReleased {
				order_id,
				seller: escrow.seller,
				amount: escrow.amount_locked,
			});

			Ok(())
		}

		/// Refund funds to buyer
		///
		/// - `order_id`: Order ID
		///
		/// Can only be called by DAO/Council.
		/// Unreserves funds (returns to buyer automatically).
		/// Emits `Refunded` event.
		#[pallet::call_index(2)]
		#[pallet::weight(10_000)]
		pub fn refund(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
			T::DAOOrigin::ensure_origin(origin)?;

			let mut escrow = Escrows::<T>::get(order_id).ok_or(Error::<T>::EscrowNotFound)?;

			// Validate status
			ensure!(
				escrow.status == EscrowStatus::Locked,
				Error::<T>::InvalidStatus
			);

			// Unreserve (funds return to buyer automatically)
			<T as Config>::Currency::unreserve(&escrow.buyer, escrow.amount_locked);

			// Update escrow
			escrow.status = EscrowStatus::Refunded;
			escrow.updated_at = frame_system::Pallet::<T>::block_number();

			Escrows::<T>::insert(order_id, escrow.clone());

			Self::deposit_event(Event::Refunded {
				order_id,
				buyer: escrow.buyer,
				amount: escrow.amount_locked,
			});

			Ok(())
		}

		/// Partial refund (split funds)
		///
		/// - `order_id`: Order ID
		/// - `buyer_amount`: Amount to refund to buyer
		/// - `seller_amount`: Amount to transfer to seller
		///
		/// Can only be called by DAO/Council for dispute resolution.
		/// Sum of amounts must equal locked amount.
		/// Emits `PartialRefund` event.
		#[pallet::call_index(3)]
		#[pallet::weight(10_000)]
		pub fn partial_refund(
			origin: OriginFor<T>,
			order_id: u64,
			buyer_amount: BalanceOf<T>,
			seller_amount: BalanceOf<T>,
		) -> DispatchResult {
			T::DAOOrigin::ensure_origin(origin)?;

			let mut escrow = Escrows::<T>::get(order_id).ok_or(Error::<T>::EscrowNotFound)?;

			// Validate status
			ensure!(
				escrow.status == EscrowStatus::Locked,
				Error::<T>::InvalidStatus
			);

			// Validate amounts sum correctly
			ensure!(
				buyer_amount.saturating_add(seller_amount) == escrow.amount_locked,
				Error::<T>::AmountMismatch
			);

			// Unreserve total amount
			<T as Config>::Currency::unreserve(&escrow.buyer, escrow.amount_locked);

			// Transfer seller_amount to seller (buyer keeps buyer_amount automatically)
			if seller_amount > Zero::zero() {
				<T as Config>::Currency::transfer(
					&escrow.buyer,
					&escrow.seller,
					seller_amount,
					ExistenceRequirement::KeepAlive,
				)?;
			}

			// Update escrow
			escrow.amount_released = seller_amount;
			escrow.status = EscrowStatus::PartialRefund;
			escrow.updated_at = frame_system::Pallet::<T>::block_number();

			Escrows::<T>::insert(order_id, escrow);

			Self::deposit_event(Event::PartialRefund {
				order_id,
				buyer_amount,
				seller_amount,
			});

			Ok(())
		}

		/// Mark escrow as disputed (FASE 6 - NEW)
		///
		/// Called by bazari-dispute pallet when a dispute is opened.
		/// Prevents auto-release while dispute is active.
		///
		/// - `order_id`: Order ID
		///
		/// Can only be called by root (for now, later DisputeOrigin).
		/// Emits `EscrowDisputed` event.
		#[pallet::call_index(4)]
		#[pallet::weight(10_000)]
		pub fn mark_disputed(
			origin: OriginFor<T>,
			order_id: u64,
		) -> DispatchResult {
			// For now, require root. Later can be changed to DisputeOrigin
			// that the dispute pallet can use
			ensure_root(origin)?;

			let mut escrow = Escrows::<T>::get(order_id).ok_or(Error::<T>::EscrowNotFound)?;

			// Only locked escrows can be disputed
			ensure!(
				escrow.status == EscrowStatus::Locked,
				Error::<T>::InvalidStatus
			);

			// Mark as disputed
			escrow.status = EscrowStatus::Disputed;
			escrow.updated_at = frame_system::Pallet::<T>::block_number();

			Escrows::<T>::insert(order_id, escrow);

			Self::deposit_event(Event::EscrowDisputed { order_id });

			Ok(())
		}
	}

	// ============================================
	// INTERNAL FUNCTIONS (for cross-pallet calls)
	// ============================================

	impl<T: Config> Pallet<T> {
		/// Mark escrow as disputed (internal function for cross-pallet calls)
		///
		/// This can be called directly by the dispute pallet without going through
		/// the extrinsic layer.
		///
		/// Returns error if escrow not found or not in Locked status.
		pub fn mark_disputed_internal(order_id: u64) -> DispatchResult {
			let mut escrow = Escrows::<T>::get(order_id).ok_or(Error::<T>::EscrowNotFound)?;

			// Only locked escrows can be disputed
			ensure!(
				escrow.status == EscrowStatus::Locked,
				Error::<T>::InvalidStatus
			);

			// Mark as disputed
			escrow.status = EscrowStatus::Disputed;
			escrow.updated_at = frame_system::Pallet::<T>::block_number();

			Escrows::<T>::insert(order_id, escrow);

			Self::deposit_event(Event::EscrowDisputed { order_id });

			Ok(())
		}
	}
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{
		assert_noop, assert_ok, derive_impl, parameter_types,
		traits::{ConstU32, ConstU64},
	};
	use sp_core::H256;
	use sp_io::TestExternalities;
	use sp_runtime::{
		traits::{BlakeTwo256, IdentityLookup},
		BuildStorage,
	};

	type AccountId = u64;
	type Balance = u128;
	type BlockNumber = u32;

	frame_support::construct_runtime!(
		pub enum Test {
			System: frame_system,
			Balances: pallet_balances,
			Escrow: pallet,
		}
	);

	parameter_types! {
		pub const BlockHashCount: BlockNumber = 2400;
		pub const ExistentialDeposit: Balance = 1;
		pub const MaxLocks: u32 = 50;
		pub const MaxReserves: u32 = 50;
	}

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
	impl frame_system::Config for Test {
		type RuntimeEvent = RuntimeEvent;
		type RuntimeCall = RuntimeCall;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<AccountId>;
		type Block = frame_system::mocking::MockBlock<Test>;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountData = pallet_balances::AccountData<Balance>;
	}

	impl pallet_balances::Config for Test {
		type Balance = Balance;
		type DustRemoval = ();
		type RuntimeEvent = RuntimeEvent;
		type ExistentialDeposit = ExistentialDeposit;
		type AccountStore = System;
		type WeightInfo = ();
		type MaxLocks = MaxLocks;
		type MaxReserves = MaxReserves;
		type ReserveIdentifier = [u8; 8];
		type RuntimeHoldReason = ();
		type RuntimeFreezeReason = ();
		type DoneSlashHandler = ();
		type FreezeIdentifier = ();
		type MaxFreezes = ConstU32<0>;
	}

	impl Config for Test {
		type RuntimeEvent = RuntimeEvent;
		type Currency = Balances;
		type DAOOrigin = frame_system::EnsureRoot<AccountId>;
	}

	fn new_test_ext() -> TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default()
			.build_storage()
			.unwrap();
		pallet_balances::GenesisConfig::<Test> {
			balances: vec![(1, 1_000_000), (2, 1_000_000), (99, 1_000_000)],
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
		});
		ext
	}

	#[test]
	fn lock_funds_works() {
		new_test_ext().execute_with(|| {
			let buyer = 1;
			let seller = 2;
			let amount = 1000;

			let order_id = 1;

			// Lock funds
			assert_ok!(Pallet::<Test>::lock_funds(
				frame_system::RawOrigin::Signed(buyer).into(),
				order_id,
				seller,
				amount
			));

			// Verify escrow created
			let escrow = Pallet::<Test>::escrows(order_id).unwrap();
			assert_eq!(escrow.order_id, order_id);
			assert_eq!(escrow.buyer, buyer);
			assert_eq!(escrow.seller, seller);
			assert_eq!(escrow.amount_locked, amount);
			assert_eq!(escrow.status, EscrowStatus::Locked);

			// Verify funds reserved
			assert_eq!(Balances::reserved_balance(buyer), amount);
			assert_eq!(Balances::free_balance(buyer), 1_000_000 - amount);
		});
	}

	#[test]
	fn release_funds_works() {
		new_test_ext().execute_with(|| {
			let buyer = 1;
			let seller = 2;
			let amount = 1000;

			let order_id = 1;

			// Lock funds
			assert_ok!(Pallet::<Test>::lock_funds(
				frame_system::RawOrigin::Signed(buyer).into(),
				order_id,
				seller,
				amount
			));

			let seller_balance_before = Balances::free_balance(seller);

			// Release funds
			assert_ok!(Pallet::<Test>::release_funds(
				frame_system::RawOrigin::Signed(buyer).into(),
				order_id
			));

			// Verify escrow updated
			let escrow = Pallet::<Test>::escrows(order_id).unwrap();
			assert_eq!(escrow.status, EscrowStatus::Released);
			assert_eq!(escrow.amount_released, amount);

			// Verify funds transferred
			assert_eq!(Balances::reserved_balance(buyer), 0);
			assert_eq!(Balances::free_balance(seller), seller_balance_before + amount);
		});
	}

	#[test]
	fn release_funds_fails_unauthorized() {
		new_test_ext().execute_with(|| {
			let buyer = 1;
			let seller = 2;
			let amount = 1000;

			let order_id = 1;

			assert_ok!(Pallet::<Test>::lock_funds(
				frame_system::RawOrigin::Signed(buyer).into(),
				order_id,
				seller,
				amount
			));

			// Seller tries to release (only buyer can)
			assert_noop!(
				Pallet::<Test>::release_funds(frame_system::RawOrigin::Signed(seller).into(), order_id),
				Error::<Test>::Unauthorized
			);
		});
	}

	#[test]
	fn refund_works() {
		new_test_ext().execute_with(|| {
			let buyer = 1;
			let seller = 2;
			let amount = 1000;

			let order_id = 1;

			assert_ok!(Pallet::<Test>::lock_funds(
				frame_system::RawOrigin::Signed(buyer).into(),
				order_id,
				seller,
				amount
			));

			let buyer_balance_before = Balances::free_balance(buyer);

			// Refund (as DAO/Root)
			assert_ok!(Pallet::<Test>::refund(frame_system::RawOrigin::Root.into(), order_id));

			// Verify escrow updated
			let escrow = Pallet::<Test>::escrows(order_id).unwrap();
			assert_eq!(escrow.status, EscrowStatus::Refunded);

			// Verify funds returned to buyer
			assert_eq!(Balances::reserved_balance(buyer), 0);
			assert_eq!(Balances::free_balance(buyer), buyer_balance_before + amount);
		});
	}

	#[test]
	fn partial_refund_works() {
		new_test_ext().execute_with(|| {
			let buyer = 1;
			let seller = 2;
			let amount = 1000;
			let buyer_refund = 600;
			let seller_payment = 400;

			let order_id = 1;

			assert_ok!(Pallet::<Test>::lock_funds(
				frame_system::RawOrigin::Signed(buyer).into(),
				order_id,
				seller,
				amount
			));

			let buyer_balance_before = Balances::free_balance(buyer);
			let seller_balance_before = Balances::free_balance(seller);

			// Partial refund (as DAO/Root)
			assert_ok!(Pallet::<Test>::partial_refund(
				frame_system::RawOrigin::Root.into(),
				order_id,
				buyer_refund,
				seller_payment
			));

			// Verify escrow updated
			let escrow = Pallet::<Test>::escrows(order_id).unwrap();
			assert_eq!(escrow.status, EscrowStatus::PartialRefund);
			assert_eq!(escrow.amount_released, seller_payment);

			// Verify funds split correctly
			assert_eq!(Balances::reserved_balance(buyer), 0);
			assert_eq!(Balances::free_balance(buyer), buyer_balance_before + buyer_refund);
			assert_eq!(Balances::free_balance(seller), seller_balance_before + seller_payment);
		});
	}

	#[test]
	fn partial_refund_fails_amount_mismatch() {
		new_test_ext().execute_with(|| {
			let buyer = 1;
			let seller = 2;
			let amount = 1000;

			let order_id = 1;

			assert_ok!(Pallet::<Test>::lock_funds(
				frame_system::RawOrigin::Signed(buyer).into(),
				order_id,
				seller,
				amount
			));

			// Try partial refund with wrong amounts
			assert_noop!(
				Pallet::<Test>::partial_refund(
					frame_system::RawOrigin::Root.into(),
					order_id,
					500,
					400 // Sum is 900, not 1000
				),
				Error::<Test>::AmountMismatch
			);
		});
	}

	#[test]
	fn double_release_fails() {
		new_test_ext().execute_with(|| {
			let buyer = 1;
			let seller = 2;
			let amount = 1000;

			let order_id = 1;

			assert_ok!(Pallet::<Test>::lock_funds(
				frame_system::RawOrigin::Signed(buyer).into(),
				order_id,
				seller,
				amount
			));

			assert_ok!(Pallet::<Test>::release_funds(
				frame_system::RawOrigin::Signed(buyer).into(),
				order_id
			));

			// Try to release again
			assert_noop!(
				Pallet::<Test>::release_funds(frame_system::RawOrigin::Signed(buyer).into(), order_id),
				Error::<Test>::InvalidStatus
			);
		});
	}
}
