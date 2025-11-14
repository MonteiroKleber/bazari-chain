#![cfg_attr(not(feature = "std"), no_std)]

//! # Bazari Fulfillment Pallet
//!
//! This pallet manages courier registry, reputation, staking, and delivery tracking.
//!
//! ## Overview
//!
//! The Bazari Fulfillment pallet provides functionality for:
//! - Courier registration with mandatory staking (1000 BZR minimum)
//! - Reputation scoring (0-1000) based on delivery performance
//! - Order assignment and completion tracking
//! - Slashing mechanism for disputes (DAO-controlled)
//! - Merkle root anchoring for off-chain reviews

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, ReservableCurrency},
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{Saturating, Zero};

	#[cfg(not(feature = "std"))]
	use alloc::vec::Vec;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Courier information stored on-chain
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct Courier<T: Config> {
		/// Courier's account
		pub account: T::AccountId,
		/// Staked amount (reserved/locked)
		pub stake: BalanceOf<T>,
		/// Reputation score (0-1000)
		pub reputation_score: u32,
		/// Service areas (H3 geohash)
		pub service_areas: BoundedVec<u64, T::MaxServiceAreas>,
		/// Total deliveries assigned
		pub total_deliveries: u32,
		/// Successful deliveries completed
		pub successful_deliveries: u32,
		/// Disputed deliveries
		pub disputed_deliveries: u32,
		/// Whether courier is active
		pub is_active: bool,
		/// Block number when registered
		pub registered_at: BlockNumberFor<T>,
		/// Merkle root of off-chain reviews
		pub reviews_merkle_root: [u8; 32],
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_bazari_identity::Config {
		/// The overarching event type
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Currency for staking
		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		/// Minimum stake required to register as courier
		#[pallet::constant]
		type MinCourierStake: Get<BalanceOf<Self>>;

		/// Maximum number of service areas per courier
		#[pallet::constant]
		type MaxServiceAreas: Get<u32>;

		/// Maximum deliveries per courier
		#[pallet::constant]
		type MaxDeliveriesPerCourier: Get<u32>;

		/// DAO origin for slashing and governance
		type DAOOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for extrinsics
		type WeightInfo: WeightInfo;
	}

	/// Registered couriers
	#[pallet::storage]
	#[pallet::getter(fn couriers)]
	pub type Couriers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Courier<T>>;

	/// Mapping from order ID to assigned courier
	#[pallet::storage]
	#[pallet::getter(fn order_couriers)]
	pub type OrderCouriers<T: Config> = StorageMap<_, Blake2_128Concat, u64, T::AccountId>;

	/// Deliveries assigned to each courier
	#[pallet::storage]
	#[pallet::getter(fn courier_deliveries)]
	pub type CourierDeliveries<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<u64, T::MaxDeliveriesPerCourier>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Courier registered
		CourierRegistered { account: T::AccountId, stake: BalanceOf<T> },
		/// Courier assigned to order
		CourierAssigned { order_id: u64, courier: T::AccountId },
		/// Delivery completed
		DeliveryCompleted { order_id: u64, courier: T::AccountId },
		/// Courier slashed
		CourierSlashed {
			courier: T::AccountId,
			amount: BalanceOf<T>,
			reason: BoundedVec<u8, ConstU32<128>>,
		},
		/// Reviews Merkle root updated
		ReviewsMerkleRootUpdated { courier: T::AccountId, merkle_root: [u8; 32] },
		/// Courier deactivated
		CourierDeactivated { account: T::AccountId },
		/// Courier reactivated
		CourierReactivated { account: T::AccountId },
		/// Courier stake increased
		CourierStakeIncreased { account: T::AccountId, new_stake: BalanceOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Courier not found
		CourierNotFound,
		/// Insufficient stake
		InsufficientStake,
		/// Too many service areas
		TooManyServiceAreas,
		/// Courier is inactive
		CourierInactive,
		/// Order not assigned to any courier
		OrderNotAssigned,
		/// Unauthorized action
		Unauthorized,
		/// Too many deliveries for this courier
		TooManyDeliveries,
		/// Courier already registered
		CourierAlreadyRegistered,
		/// Identity required to register as courier
		IdentityRequired,
		/// Reputation score out of range
		ReputationOutOfRange,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register as a courier with stake
		///
		/// # Arguments
		/// * `origin` - The account registering as courier
		/// * `stake` - Amount to stake (must be >= MinCourierStake)
		/// * `service_areas` - H3 geohash service areas (max 10)
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::register_courier())]
		pub fn register_courier(
			origin: OriginFor<T>,
			stake: BalanceOf<T>,
			service_areas: Vec<u64>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Ensure identity exists
			ensure!(
				pallet_bazari_identity::Pallet::<T>::has_profile(&who),
				Error::<T>::IdentityRequired
			);

			// Ensure not already registered
			ensure!(!Couriers::<T>::contains_key(&who), Error::<T>::CourierAlreadyRegistered);

			// Validate stake amount
			ensure!(stake >= T::MinCourierStake::get(), Error::<T>::InsufficientStake);

			// Reserve stake
			T::Currency::reserve(&who, stake)?;

			// Convert service areas to bounded vec
			let bounded_areas: BoundedVec<u64, T::MaxServiceAreas> =
				service_areas.try_into().map_err(|_| Error::<T>::TooManyServiceAreas)?;

			// Create courier
			let courier = Courier {
				account: who.clone(),
				stake,
				reputation_score: 500, // Start at median
				service_areas: bounded_areas,
				total_deliveries: 0,
				successful_deliveries: 0,
				disputed_deliveries: 0,
				is_active: true,
				registered_at: <frame_system::Pallet<T>>::block_number(),
				reviews_merkle_root: [0u8; 32], // Empty initially
			};

			Couriers::<T>::insert(&who, courier);

			Self::deposit_event(Event::CourierRegistered { account: who, stake });

			Ok(())
		}

		/// Assign courier to an order
		///
		/// # Arguments
		/// * `origin` - Signed origin (system or seller)
		/// * `order_id` - Order ID to assign
		/// * `courier` - Courier account to assign
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::assign_courier())]
		pub fn assign_courier(
			origin: OriginFor<T>,
			order_id: u64,
			courier: T::AccountId,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			// Validate courier exists and is active
			let courier_data =
				Couriers::<T>::get(&courier).ok_or(Error::<T>::CourierNotFound)?;

			ensure!(courier_data.is_active, Error::<T>::CourierInactive);
			ensure!(
				courier_data.stake >= T::MinCourierStake::get(),
				Error::<T>::InsufficientStake
			);

			// Assign courier to order
			OrderCouriers::<T>::insert(order_id, &courier);

			// Track delivery in courier's list
			CourierDeliveries::<T>::try_mutate(&courier, |deliveries| {
				deliveries.try_push(order_id).map_err(|_| Error::<T>::TooManyDeliveries)
			})?;

			Self::deposit_event(Event::CourierAssigned { order_id, courier });

			Ok(())
		}

		/// Complete a delivery
		///
		/// # Arguments
		/// * `origin` - Courier assigned to the order
		/// * `order_id` - Order ID to mark as completed
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::complete_delivery())]
		pub fn complete_delivery(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
			let courier = ensure_signed(origin)?;

			// Validate courier is assigned to this order
			let assigned_courier =
				OrderCouriers::<T>::get(order_id).ok_or(Error::<T>::OrderNotAssigned)?;

			ensure!(courier == assigned_courier, Error::<T>::Unauthorized);

			// Update courier stats
			Couriers::<T>::try_mutate(&courier, |maybe_courier| {
				let courier_data = maybe_courier.as_mut().ok_or(Error::<T>::CourierNotFound)?;

				courier_data.total_deliveries = courier_data.total_deliveries.saturating_add(1);
				courier_data.successful_deliveries =
					courier_data.successful_deliveries.saturating_add(1);

				// Update reputation (+10 points per successful delivery, max 1000)
				courier_data.reputation_score =
					courier_data.reputation_score.saturating_add(10).min(1000);

				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::DeliveryCompleted { order_id, courier });

			Ok(())
		}

		/// Slash a courier (DAO only)
		///
		/// # Arguments
		/// * `origin` - DAO origin
		/// * `courier` - Courier to slash
		/// * `slash_amount` - Amount to slash from stake
		/// * `reason` - Reason for slashing
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::slash_courier())]
		pub fn slash_courier(
			origin: OriginFor<T>,
			courier: T::AccountId,
			slash_amount: BalanceOf<T>,
			reason: Vec<u8>,
		) -> DispatchResult {
			T::DAOOrigin::ensure_origin(origin)?;

			Couriers::<T>::try_mutate(&courier, |maybe_courier| {
				let courier_data = maybe_courier.as_mut().ok_or(Error::<T>::CourierNotFound)?;

				// Slash reserved stake (min of slash_amount and current stake)
				let slashed = slash_amount.min(courier_data.stake);
				let _remaining = T::Currency::slash_reserved(&courier, slashed).0;

				// Update courier data
				courier_data.stake = courier_data.stake.saturating_sub(slashed);
				courier_data.reputation_score = courier_data.reputation_score.saturating_sub(100);
				courier_data.disputed_deliveries =
					courier_data.disputed_deliveries.saturating_add(1);

				// Deactivate if stake too low
				if courier_data.stake < T::MinCourierStake::get() {
					courier_data.is_active = false;
					Self::deposit_event(Event::CourierDeactivated { account: courier.clone() });
				}

				let bounded_reason: BoundedVec<u8, ConstU32<128>> =
					reason.try_into().unwrap_or_default();

				Self::deposit_event(Event::CourierSlashed {
					courier: courier.clone(),
					amount: slashed,
					reason: bounded_reason,
				});

				Ok::<(), DispatchError>(())
			})
		}

		/// Update reviews Merkle root (Root only)
		///
		/// # Arguments
		/// * `origin` - Root origin (backend worker)
		/// * `courier` - Courier account
		/// * `new_merkle_root` - New Merkle root hash
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::update_reviews_merkle_root())]
		pub fn update_reviews_merkle_root(
			origin: OriginFor<T>,
			courier: T::AccountId,
			new_merkle_root: [u8; 32],
		) -> DispatchResult {
			ensure_root(origin)?;

			Couriers::<T>::try_mutate(&courier, |maybe_courier| {
				let courier_data = maybe_courier.as_mut().ok_or(Error::<T>::CourierNotFound)?;

				// Update Merkle root
				courier_data.reviews_merkle_root = new_merkle_root;

				Self::deposit_event(Event::ReviewsMerkleRootUpdated {
					courier: courier.clone(),
					merkle_root: new_merkle_root,
				});

				Ok::<(), DispatchError>(())
			})
		}

		/// Deactivate courier (self-initiated)
		///
		/// # Arguments
		/// * `origin` - Courier account
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::deactivate_courier())]
		pub fn deactivate_courier(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Couriers::<T>::try_mutate(&who, |maybe_courier| {
				let courier_data = maybe_courier.as_mut().ok_or(Error::<T>::CourierNotFound)?;

				courier_data.is_active = false;

				Self::deposit_event(Event::CourierDeactivated { account: who.clone() });

				Ok::<(), DispatchError>(())
			})
		}

		/// Reactivate courier
		///
		/// # Arguments
		/// * `origin` - Courier account
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::reactivate_courier())]
		pub fn reactivate_courier(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Couriers::<T>::try_mutate(&who, |maybe_courier| {
				let courier_data = maybe_courier.as_mut().ok_or(Error::<T>::CourierNotFound)?;

				// Ensure stake is sufficient
				ensure!(
					courier_data.stake >= T::MinCourierStake::get(),
					Error::<T>::InsufficientStake
				);

				courier_data.is_active = true;

				Self::deposit_event(Event::CourierReactivated { account: who.clone() });

				Ok::<(), DispatchError>(())
			})
		}

		/// Increase courier stake
		///
		/// # Arguments
		/// * `origin` - Courier account
		/// * `additional_stake` - Additional amount to stake
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::increase_stake())]
		pub fn increase_stake(
			origin: OriginFor<T>,
			additional_stake: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!additional_stake.is_zero(), Error::<T>::InsufficientStake);

			Couriers::<T>::try_mutate(&who, |maybe_courier| {
				let courier_data = maybe_courier.as_mut().ok_or(Error::<T>::CourierNotFound)?;

				// Reserve additional stake
				T::Currency::reserve(&who, additional_stake)?;

				// Update stake
				courier_data.stake = courier_data.stake.saturating_add(additional_stake);

				Self::deposit_event(Event::CourierStakeIncreased {
					account: who.clone(),
					new_stake: courier_data.stake,
				});

				Ok::<(), DispatchError>(())
			})
		}
	}

	/// Weight information for extrinsics
	pub trait WeightInfo {
		fn register_courier() -> Weight;
		fn assign_courier() -> Weight;
		fn complete_delivery() -> Weight;
		fn slash_courier() -> Weight;
		fn update_reviews_merkle_root() -> Weight;
		fn deactivate_courier() -> Weight;
		fn reactivate_courier() -> Weight;
		fn increase_stake() -> Weight;
	}

	impl WeightInfo for () {
		fn register_courier() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn assign_courier() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn complete_delivery() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn slash_courier() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn update_reviews_merkle_root() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn deactivate_courier() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn reactivate_courier() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn increase_stake() -> Weight {
			Weight::from_parts(10_000, 0)
		}
	}
}
