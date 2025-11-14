#![cfg_attr(not(feature = "std"), no_std)]

//! # Bazari Rewards Pallet
//!
//! Pallet for minting ZARI tokens as cashback and managing on-chain missions/quests.
//!
//! ## Overview
//!
//! This pallet provides:
//! - **Cashback System**: Mint ZARI tokens (AssetId 1) as cashback after purchases
//! - **Missions/Quests**: On-chain mission system with progress tracking and rewards
//! - **Tiered Rates**: Configurable cashback rates based on purchase amount
//!
//! ## Interface
//!
//! ### Extrinsics
//!
//! - `mint_cashback` - Mint ZARI cashback (root/backend only)
//! - `create_mission` - Create new mission (DAO only)
//! - `update_progress` - Update user mission progress (root/backend only)
//! - `claim_reward` - Claim mission reward (users)
//!
//! ### Storage
//!
//! - `Missions` - Mission definitions
//! - `UserProgress` - User progress per mission
//! - `CashbackRates` - Tiered cashback rates
//! - `MissionIdCounter` - Auto-increment mission IDs

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		traits::fungibles::{Inspect, Mutate},
	};
	use frame_system::pallet_prelude::*;
	use serde::{Deserialize, Serialize};

	/// Balance type for asset amounts
	pub type BalanceOf<T> = <<T as Config>::Assets as Inspect<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	/// Asset ID type
	pub type AssetIdOf<T> = <<T as Config>::Assets as Inspect<
		<T as frame_system::Config>::AccountId,
	>>::AssetId;

	/// Mission type enumeration
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum MissionType {
		/// Complete first purchase
		FirstPurchase,
		/// Refer a friend
		ReferFriend,
		/// Complete N orders
		CompleteNOrders(u32),
		/// Spend X amount
		SpendAmount(u128),
		/// Daily login for N days
		DailyLogin(u32),
	}

	/// Mission definition
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct Mission<T: Config> {
		/// Unique mission ID
		pub mission_id: u64,
		/// Mission title (max 64 bytes)
		pub title: BoundedVec<u8, ConstU32<64>>,
		/// Mission description (max 256 bytes)
		pub description: BoundedVec<u8, ConstU32<256>>,
		/// Mission type
		pub mission_type: MissionType,
		/// Reward amount in ZARI
		pub reward_amount: BalanceOf<T>,
		/// Required count to complete
		pub required_count: u32,
		/// Is mission active
		pub is_active: bool,
		/// Creation block
		pub created_at: BlockNumberFor<T>,
	}

	/// User progress tracking
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Progress<T: Config> {
		/// Current progress count
		pub current_count: u32,
		/// Is mission completed
		pub is_completed: bool,
		/// Is reward claimed
		pub is_claimed: bool,
		/// Completion block
		pub completed_at: Option<BlockNumberFor<T>>,
	}

	/// Cashback rate tier (threshold, percentage)
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Serialize, Deserialize)]
	pub struct CashbackTier {
		/// Amount threshold in BZR (using u128 for simplicity)
		pub threshold: u128,
		/// Cashback percentage (e.g., 2 = 2%)
		pub percentage: u32,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset management trait (pallet-assets)
		type Assets: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

		/// ZARI asset ID
		#[pallet::constant]
		type ZariAssetId: Get<AssetIdOf<Self>>;

		/// DAO origin for mission management
		type DAOOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight info (placeholder)
		type WeightInfo: WeightInfo;
	}

	/// Placeholder for weight info
	pub trait WeightInfo {
		fn mint_cashback() -> Weight;
		fn create_mission() -> Weight;
		fn update_progress() -> Weight;
		fn claim_reward() -> Weight;
	}

	impl WeightInfo for () {
		fn mint_cashback() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn create_mission() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn update_progress() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn claim_reward() -> Weight {
			Weight::from_parts(10_000, 0)
		}
	}

	// ============================================
	// STORAGE
	// ============================================

	/// Mission definitions storage
	#[pallet::storage]
	#[pallet::getter(fn missions)]
	pub type Missions<T: Config> = StorageMap<_, Blake2_128Concat, u64, Mission<T>>;

	/// User progress per mission
	#[pallet::storage]
	#[pallet::getter(fn user_progress)]
	pub type UserProgress<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		u64,
		Progress<T>,
	>;

	/// Cashback rate tiers
	#[pallet::storage]
	#[pallet::getter(fn cashback_rates)]
	pub type CashbackRates<T: Config> =
		StorageValue<_, BoundedVec<CashbackTier, ConstU32<10>>, ValueQuery>;

	/// Mission ID counter
	#[pallet::storage]
	#[pallet::getter(fn mission_id_counter)]
	pub type MissionIdCounter<T: Config> = StorageValue<_, u64, ValueQuery>;

	// ============================================
	// GENESIS CONFIG
	// ============================================

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		/// Initial cashback tiers
		pub cashback_tiers: Vec<CashbackTier>,
		/// Phantom data
		pub _phantom: PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				// Default tiers: 0-99 = 1%, 100-499 = 2%, 500+ = 3%
				cashback_tiers: vec![
					CashbackTier { threshold: 0, percentage: 1 },                  // 0-99 BZR
					CashbackTier { threshold: 100_000_000_000_000, percentage: 2 }, // 100-499 BZR
					CashbackTier { threshold: 500_000_000_000_000, percentage: 3 }, // 500+ BZR
				],
				_phantom: PhantomData,
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			let bounded_tiers: BoundedVec<CashbackTier, ConstU32<10>> =
				self.cashback_tiers.clone().try_into().expect("Too many tiers");
			CashbackRates::<T>::put(bounded_tiers);
		}
	}

	// ============================================
	// EVENTS
	// ============================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Cashback minted
		CashbackMinted {
			user: T::AccountId,
			amount: BalanceOf<T>,
			order_amount: u128,
		},
		/// Mission created
		MissionCreated { mission_id: u64 },
		/// Mission completed
		MissionCompleted { user: T::AccountId, mission_id: u64 },
		/// Reward claimed
		RewardClaimed {
			user: T::AccountId,
			mission_id: u64,
			amount: BalanceOf<T>,
		},
	}

	// ============================================
	// ERRORS
	// ============================================

	#[pallet::error]
	pub enum Error<T> {
		/// Mission not found
		MissionNotFound,
		/// Mission is inactive
		MissionInactive,
		/// Progress not found
		ProgressNotFound,
		/// Mission not completed
		MissionNotCompleted,
		/// Reward already claimed
		AlreadyClaimed,
		/// Mission title too long
		TitleTooLong,
		/// Mission description too long
		DescriptionTooLong,
		/// Invalid cashback amount
		InvalidCashbackAmount,
		/// Invalid mission type
		InvalidMissionType,
	}

	// ============================================
	// EXTRINSICS
	// ============================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Mint ZARI cashback to buyer (root/backend only)
		///
		/// - `buyer`: Account to receive cashback
		/// - `order_amount`: Total order amount in BZR (used to calculate percentage)
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::mint_cashback())]
		pub fn mint_cashback(
			origin: OriginFor<T>,
			buyer: T::AccountId,
			order_amount: u128,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(order_amount > 0, Error::<T>::InvalidCashbackAmount);

			// Calculate cashback rate
			let rate = Self::get_cashback_rate(order_amount);

			// Calculate cashback amount
			let cashback_amount = Self::calculate_cashback(order_amount, rate)?;

			// Mint ZARI tokens to buyer
			T::Assets::mint_into(T::ZariAssetId::get(), &buyer, cashback_amount)?;

			Self::deposit_event(Event::CashbackMinted {
				user: buyer,
				amount: cashback_amount,
				order_amount,
			});

			Ok(())
		}

		/// Create new mission (DAO only)
		///
		/// - `title`: Mission title (max 64 bytes)
		/// - `description`: Mission description (max 256 bytes)
		/// - `mission_type_id`: Type of mission (0=FirstPurchase, 1=ReferFriend, etc)
		/// - `mission_type_value`: Value for types that need it (e.g., N for CompleteNOrders)
		/// - `reward_amount`: Reward in ZARI
		/// - `required_count`: Count needed to complete
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::create_mission())]
		pub fn create_mission(
			origin: OriginFor<T>,
			title: Vec<u8>,
			description: Vec<u8>,
			mission_type_id: u8,
			mission_type_value: u32,
			reward_amount: BalanceOf<T>,
			required_count: u32,
		) -> DispatchResult {
			T::DAOOrigin::ensure_origin(origin)?;

			let mission_id = MissionIdCounter::<T>::get();
			MissionIdCounter::<T>::put(mission_id.saturating_add(1));

			let bounded_title: BoundedVec<u8, ConstU32<64>> =
				title.try_into().map_err(|_| Error::<T>::TitleTooLong)?;
			let bounded_description: BoundedVec<u8, ConstU32<256>> =
				description.try_into().map_err(|_| Error::<T>::DescriptionTooLong)?;

			// Parse mission type from u8
			let mission_type = match mission_type_id {
				0 => MissionType::FirstPurchase,
				1 => MissionType::ReferFriend,
				2 => MissionType::CompleteNOrders(mission_type_value),
				3 => MissionType::SpendAmount(mission_type_value as u128),
				4 => MissionType::DailyLogin(mission_type_value),
				_ => return Err(Error::<T>::InvalidMissionType.into()),
			};

			let mission = Mission {
				mission_id,
				title: bounded_title,
				description: bounded_description,
				mission_type,
				reward_amount,
				required_count,
				is_active: true,
				created_at: frame_system::Pallet::<T>::block_number(),
			};

			Missions::<T>::insert(mission_id, mission);

			Self::deposit_event(Event::MissionCreated { mission_id });

			Ok(())
		}

		/// Update user progress (root/backend only)
		///
		/// - `user`: User account
		/// - `mission_id`: Mission ID
		/// - `increment`: Amount to increment progress
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::update_progress())]
		pub fn update_progress(
			origin: OriginFor<T>,
			user: T::AccountId,
			mission_id: u64,
			increment: u32,
		) -> DispatchResult {
			ensure_root(origin)?;

			let mission = Missions::<T>::get(mission_id).ok_or(Error::<T>::MissionNotFound)?;

			ensure!(mission.is_active, Error::<T>::MissionInactive);

			UserProgress::<T>::mutate(&user, mission_id, |maybe_progress| {
				let mut progress = maybe_progress.clone().unwrap_or(Progress {
					current_count: 0,
					is_completed: false,
					is_claimed: false,
					completed_at: None,
				});

				progress.current_count = progress.current_count.saturating_add(increment);

				if progress.current_count >= mission.required_count && !progress.is_completed {
					progress.is_completed = true;
					progress.completed_at = Some(frame_system::Pallet::<T>::block_number());

					Self::deposit_event(Event::MissionCompleted {
						user: user.clone(),
						mission_id,
					});
				}

				*maybe_progress = Some(progress);
			});

			Ok(())
		}

		/// Claim mission reward (users)
		///
		/// - `mission_id`: Mission ID to claim
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::claim_reward())]
		pub fn claim_reward(origin: OriginFor<T>, mission_id: u64) -> DispatchResult {
			let user = ensure_signed(origin)?;

			let mission = Missions::<T>::get(mission_id).ok_or(Error::<T>::MissionNotFound)?;

			let mut progress =
				UserProgress::<T>::get(&user, mission_id).ok_or(Error::<T>::ProgressNotFound)?;

			ensure!(progress.is_completed, Error::<T>::MissionNotCompleted);
			ensure!(!progress.is_claimed, Error::<T>::AlreadyClaimed);

			// Mint ZARI reward
			T::Assets::mint_into(T::ZariAssetId::get(), &user, mission.reward_amount)?;

			progress.is_claimed = true;
			UserProgress::<T>::insert(&user, mission_id, progress);

			Self::deposit_event(Event::RewardClaimed {
				user,
				mission_id,
				amount: mission.reward_amount,
			});

			Ok(())
		}
	}

	// ============================================
	// HELPER FUNCTIONS
	// ============================================

	impl<T: Config> Pallet<T> {
		/// Get cashback rate based on order amount
		pub fn get_cashback_rate(order_amount: u128) -> u32 {
			let rates = CashbackRates::<T>::get();

			// Find the highest tier where order_amount >= threshold
			let mut rate = 1; // Default 1%
			for tier in rates.iter() {
				if order_amount >= tier.threshold {
					rate = tier.percentage;
				} else {
					break;
				}
			}

			rate
		}

		/// Calculate cashback amount
		pub fn calculate_cashback(order_amount: u128, rate: u32) -> Result<BalanceOf<T>, DispatchError> {
			let cashback = order_amount
				.checked_mul(rate as u128)
				.and_then(|v| v.checked_div(100))
				.ok_or(Error::<T>::InvalidCashbackAmount)?;

			// Convert u128 to BalanceOf<T>
			cashback.try_into().map_err(|_| Error::<T>::InvalidCashbackAmount.into())
		}
	}
}
