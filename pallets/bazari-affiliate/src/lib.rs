#![cfg_attr(not(feature = "std"), no_std)]

//! # Bazari Affiliate Pallet
//!
//! Multi-level affiliate commission system with DAG and automatic decay.
//!
//! ## Overview
//!
//! The Bazari Affiliate pallet provides:
//! - DAG of referrals (up to 5 levels deep)
//! - 50% decay per level (L0: 5%, L1: 2.5%, L2: 1.25%, L3: 0.62%, L4: 0.31%)
//! - Anti-gaming: prevent self-referral and circular references
//! - Automatic commission distribution on order completion
//! - Privacy-preserving Merkle root proofs

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
		traits::{Currency, ExistenceRequirement::KeepAlive},
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{Saturating, Zero};

	#[cfg(not(feature = "std"))]
	use alloc::vec::Vec;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Affiliate statistics stored on-chain
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
	pub struct AffiliateStats<Balance> {
		/// Total number of referrals (direct + indirect)
		pub total_referrals: u32,
		/// Direct referrals only
		pub direct_referrals: u32,
		/// Total commission earned
		pub total_commission_earned: Balance,
		/// Merkle root for privacy-preserving proofs
		pub merkle_root: [u8; 32],
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_bazari_commerce::Config {
		/// The overarching event type
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Currency for commission transfers
		type Currency: Currency<Self::AccountId>;

		/// Commission rates per level (in basis points)
		/// Example: [500, 250, 125, 62, 31] = 5%, 2.5%, 1.25%, 0.62%, 0.31%
		#[pallet::constant]
		type CommissionRates: Get<[u32; 5]>;

		/// Maximum referral depth (5 levels)
		#[pallet::constant]
		type MaxReferralDepth: Get<u8>;

		/// Maximum direct referrals per account
		#[pallet::constant]
		type MaxDirectReferrals: Get<u32>;

		/// Weight information for extrinsics
		type WeightInfo: WeightInfo;
	}

	/// Maps referee → referrer (one-to-one)
	#[pallet::storage]
	#[pallet::getter(fn referrer_of)]
	pub type ReferrerOf<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId, OptionQuery>;

	/// Maps referrer → list of direct referees
	#[pallet::storage]
	#[pallet::getter(fn direct_referrals)]
	pub type DirectReferrals<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<T::AccountId, T::MaxDirectReferrals>,
		ValueQuery,
	>;

	/// Affiliate statistics per account
	#[pallet::storage]
	#[pallet::getter(fn affiliate_stats)]
	pub type AffiliateStatsMap<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, AffiliateStats<BalanceOf<T>>, OptionQuery>;

	/// Commission history per order: order_id → [(affiliate, amount, level)]
	#[pallet::storage]
	#[pallet::getter(fn order_commissions)]
	pub type OrderCommissions<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		u64, // order_id
		BoundedVec<(T::AccountId, BalanceOf<T>, u8), ConstU32<5>>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Referral registered
		ReferralRegistered { referrer: T::AccountId, referee: T::AccountId },
		/// Commission distributed
		CommissionDistributed { order_id: u64, affiliate: T::AccountId, amount: BalanceOf<T>, level: u8 },
		/// Merkle root updated
		MerkleRootUpdated { account: T::AccountId, root: [u8; 32] },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Referee already has a referrer
		AlreadyReferred,
		/// Cannot refer yourself
		SelfReferral,
		/// Maximum referral depth reached
		MaxDepthReached,
		/// Too many direct referrals
		TooManyReferrals,
		/// Referrer account does not exist
		ReferrerNotFound,
		/// Circular reference detected
		CircularReference,
		/// Insufficient balance for commission
		InsufficientBalance,
		/// Order not found
		OrderNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register a referral relationship
		///
		/// # Arguments
		/// * `origin` - The referee (person being referred)
		/// * `referrer` - The referrer (person who referred)
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::register_referral())]
		pub fn register_referral(
			origin: OriginFor<T>,
			referrer: T::AccountId,
		) -> DispatchResult {
			let referee = ensure_signed(origin)?;

			// Validate self-referral
			ensure!(referee != referrer, Error::<T>::SelfReferral);

			// Validate referee not already referred
			ensure!(!ReferrerOf::<T>::contains_key(&referee), Error::<T>::AlreadyReferred);

			// Validate referrer exists
			ensure!(
				frame_system::Pallet::<T>::account_exists(&referrer),
				Error::<T>::ReferrerNotFound
			);

			// Check for circular references
			let referrer_path = Self::get_referral_path(&referrer);
			ensure!(!referrer_path.contains(&referee), Error::<T>::CircularReference);

			// Store relationship
			ReferrerOf::<T>::insert(&referee, &referrer);

			// Update direct referrals list
			DirectReferrals::<T>::try_mutate(&referrer, |referrals| {
				referrals.try_push(referee.clone()).map_err(|_| Error::<T>::TooManyReferrals)
			})?;

			// Update stats
			AffiliateStatsMap::<T>::mutate(&referrer, |maybe_stats| {
				let mut stats = maybe_stats.take().unwrap_or_default();
				stats.direct_referrals = stats.direct_referrals.saturating_add(1);
				stats.total_referrals = stats.total_referrals.saturating_add(1);
				*maybe_stats = Some(stats);
			});

			// Update ancestor stats (total_referrals)
			let mut current = referrer.clone();
			for _ in 1..T::MaxReferralDepth::get() {
				if let Some(ancestor) = ReferrerOf::<T>::get(&current) {
					AffiliateStatsMap::<T>::mutate(&ancestor, |maybe_stats| {
						let mut stats = maybe_stats.take().unwrap_or_default();
						stats.total_referrals = stats.total_referrals.saturating_add(1);
						*maybe_stats = Some(stats);
					});
					current = ancestor;
				} else {
					break;
				}
			}

			Self::deposit_event(Event::ReferralRegistered { referrer, referee });

			Ok(())
		}

		/// Distribute commissions for a completed order
		///
		/// # Arguments
		/// * `origin` - Root origin (called by system)
		/// * `order_id` - Order ID
		/// * `buyer` - Buyer account
		/// * `order_amount` - Order total amount
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::distribute_commissions())]
		pub fn distribute_commissions(
			origin: OriginFor<T>,
			order_id: u64,
			buyer: T::AccountId,
			order_amount: BalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;

			// Validate order exists
			ensure!(
				pallet_bazari_commerce::Orders::<T>::contains_key(order_id),
				Error::<T>::OrderNotFound
			);

			let buyer_clone = buyer.clone(); // Save clone for commission transfers
			let mut current_account = buyer;
			let mut commissions = Vec::new();
			let rates = T::CommissionRates::get();

			// Walk up referral tree
			for level in 0..T::MaxReferralDepth::get() {
				if let Some(referrer) = ReferrerOf::<T>::get(&current_account) {
					// Calculate commission with decay
					if let Some(&rate_bps) = rates.get(level as usize) {
						let commission = order_amount
							.saturating_mul(rate_bps.into())
							/ 10_000u32.into();

						if commission > BalanceOf::<T>::zero() {
							// Transfer commission from buyer to affiliate
							<T as Config>::Currency::transfer(&buyer_clone, &referrer, commission, KeepAlive)?;

							commissions.push((referrer.clone(), commission, level));

							// Update affiliate stats
							AffiliateStatsMap::<T>::mutate(&referrer, |maybe_stats| {
								let mut stats = maybe_stats.take().unwrap_or_default();
								stats.total_commission_earned =
									stats.total_commission_earned.saturating_add(commission);
								*maybe_stats = Some(stats);
							});

							Self::deposit_event(Event::CommissionDistributed {
								order_id,
								affiliate: referrer.clone(),
								amount: commission,
								level,
							});
						}

						current_account = referrer;
					} else {
						break;
					}
				} else {
					break;
				}
			}

			// Store commission history
			if !commissions.is_empty() {
				let _ = OrderCommissions::<T>::try_mutate(order_id, |history| {
					*history = commissions.try_into().unwrap_or_default();
					Ok::<(), DispatchError>(())
				});
			}

			Ok(())
		}

		/// Update Merkle root for an affiliate (Root only)
		///
		/// # Arguments
		/// * `origin` - Root origin
		/// * `account` - Affiliate account
		/// * `new_merkle_root` - New Merkle root hash
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::update_merkle_root())]
		pub fn update_merkle_root(
			origin: OriginFor<T>,
			account: T::AccountId,
			new_merkle_root: [u8; 32],
		) -> DispatchResult {
			ensure_root(origin)?;

			AffiliateStatsMap::<T>::mutate(&account, |maybe_stats| {
				let mut stats = maybe_stats.take().unwrap_or_default();
				stats.merkle_root = new_merkle_root;
				*maybe_stats = Some(stats);
			});

			Self::deposit_event(Event::MerkleRootUpdated { account, root: new_merkle_root });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get full referral path up to max depth
		pub fn get_referral_path(account: &T::AccountId) -> Vec<T::AccountId> {
			let mut path = Vec::new();
			let mut current = account.clone();

			for _ in 0..T::MaxReferralDepth::get() {
				if let Some(referrer) = ReferrerOf::<T>::get(&current) {
					path.push(referrer.clone());
					current = referrer;
				} else {
					break;
				}
			}

			path
		}
	}

	/// Weight information for extrinsics
	pub trait WeightInfo {
		fn register_referral() -> Weight;
		fn distribute_commissions() -> Weight;
		fn update_merkle_root() -> Weight;
	}

	impl WeightInfo for () {
		fn register_referral() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn distribute_commissions() -> Weight {
			Weight::from_parts(50_000, 0)
		}
		fn update_merkle_root() -> Weight {
			Weight::from_parts(10_000, 0)
		}
	}
}
