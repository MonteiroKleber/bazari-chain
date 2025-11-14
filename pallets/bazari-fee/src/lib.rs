#![cfg_attr(not(feature = "std"), no_std)]

//! # Bazari Fee Pallet
//!
//! Auto-splitting de pagamentos com platform fee configurável via DAO.
//!
//! ## Overview
//!
//! - Platform fee configurável (default 5%, max 10%)
//! - Auto-split: platform + affiliate + seller
//! - Toda lógica on-chain (transparente)
//! - Integration com escrow para atomic payouts

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
		traits::Currency,
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{Saturating, Zero};

	#[cfg(not(feature = "std"))]
	use alloc::vec::Vec;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Fee configuration stored on-chain
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct FeeConfiguration<AccountId, Balance> {
		/// Platform fee in basis points (500 = 5%)
		pub platform_fee_bps: u32,
		/// Treasury account receiving platform fees
		pub treasury_account: AccountId,
		/// Minimum order amount to apply fees
		pub min_order_amount: Balance,
	}

	/// Reason for split payment
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum SplitReason {
		PlatformFee,
		AffiliateCommission,
		SellerPayment,
	}

	/// Weight info trait
	pub trait WeightInfo {
		fn set_platform_fee() -> Weight;
	}

	impl WeightInfo for () {
		fn set_platform_fee() -> Weight {
			Weight::from_parts(10_000, 0)
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_bazari_commerce::Config + pallet_bazari_affiliate::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Currency<Self::AccountId>;

		/// Default platform fee (in basis points)
		#[pallet::constant]
		type DefaultPlatformFee: Get<u32>;

		/// Treasury account
		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		/// Minimum order amount
		#[pallet::constant]
		type MinOrderAmount: Get<BalanceOf<Self>>;

		/// DAO origin (for fee updates)
		type DAOOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type WeightInfo: WeightInfo;
	}

	/// Default fee configuration
	#[pallet::type_value]
	pub fn DefaultFeeConfig<T: Config>() -> FeeConfiguration<T::AccountId, BalanceOf<T>> {
		FeeConfiguration {
			platform_fee_bps: T::DefaultPlatformFee::get(),
			treasury_account: T::TreasuryAccount::get(),
			min_order_amount: T::MinOrderAmount::get(),
		}
	}

	/// Fee configuration storage
	#[pallet::storage]
	#[pallet::getter(fn fee_config)]
	pub type FeeConfig<T: Config> = StorageValue<
		_,
		FeeConfiguration<T::AccountId, BalanceOf<T>>,
		ValueQuery,
		DefaultFeeConfig<T>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Platform fee updated
		PlatformFeeUpdated {
			new_fee_bps: u32,
		},
		/// Split calculated
		SplitCalculated {
			order_id: <T as pallet_bazari_commerce::Config>::OrderId,
			total_amount: BalanceOf<T>,
			platform_fee: BalanceOf<T>,
			affiliate_commission: BalanceOf<T>,
			seller_amount: BalanceOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Fee too high (max 10%)
		FeeTooHigh,
		/// Invalid amount
		InvalidAmount,
		/// Calculation overflow
		CalculationOverflow,
		/// Order not found
		OrderNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Update platform fee (DAO only)
		///
		/// # Arguments
		/// * `new_fee_bps` - New fee in basis points (max 1000 = 10%)
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_platform_fee())]
		pub fn set_platform_fee(
			origin: OriginFor<T>,
			new_fee_bps: u32,
		) -> DispatchResult {
			T::DAOOrigin::ensure_origin(origin)?;

			// Validate fee (max 10% = 1000 bps)
			ensure!(new_fee_bps <= 1000, Error::<T>::FeeTooHigh);

			FeeConfig::<T>::mutate(|config| {
				config.platform_fee_bps = new_fee_bps;
			});

			Self::deposit_event(Event::PlatformFeeUpdated { new_fee_bps });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Calculate split for order
		///
		/// Returns: Vec<(recipient, amount, reason)>
		///
		/// # Arguments
		/// * `order_id` - Order ID for tracking
		/// * `seller` - Seller account
		/// * `buyer` - Buyer account (for affiliate lookup)
		/// * `amount` - Total order amount
		pub fn calculate_split(
			order_id: <T as pallet_bazari_commerce::Config>::OrderId,
			seller: T::AccountId,
			buyer: T::AccountId,
			amount: BalanceOf<T>,
		) -> Result<Vec<(T::AccountId, BalanceOf<T>, SplitReason)>, DispatchError> {
			let config = FeeConfig::<T>::get();
			let mut splits = Vec::new();

			// Validate amount
			ensure!(amount >= config.min_order_amount, Error::<T>::InvalidAmount);

			// 1. Platform fee
			let platform_fee = amount
				.saturating_mul(config.platform_fee_bps.into())
				/ 10_000u32.into();

			if platform_fee > BalanceOf::<T>::zero() {
				splits.push((
					config.treasury_account.clone(),
					platform_fee,
					SplitReason::PlatformFee,
				));
			}

			// 2. Affiliate commission (if exists)
			let affiliate_amount = if let Some(referrer) =
				pallet_bazari_affiliate::ReferrerOf::<T>::get(&buyer)
			{
				// Calculate L0 commission (direct referrer)
				let rates = <T as pallet_bazari_affiliate::Config>::CommissionRates::get();
				let commission = if let Some(&rate_bps) = rates.get(0) {
					amount
						.saturating_mul(rate_bps.into())
						/ 10_000u32.into()
				} else {
					BalanceOf::<T>::zero()
				};

				if commission > BalanceOf::<T>::zero() {
					splits.push((
						referrer,
						commission,
						SplitReason::AffiliateCommission,
					));
					commission
				} else {
					BalanceOf::<T>::zero()
				}
			} else {
				BalanceOf::<T>::zero()
			};

			// 3. Seller amount (remainder)
			let seller_amount = amount
				.saturating_sub(platform_fee)
				.saturating_sub(affiliate_amount);

			splits.push((
				seller,
				seller_amount,
				SplitReason::SellerPayment,
			));

			// Emit event
			Self::deposit_event(Event::SplitCalculated {
				order_id,
				total_amount: amount,
				platform_fee,
				affiliate_commission: affiliate_amount,
				seller_amount,
			});

			Ok(splits)
		}

		/// Get current platform fee in basis points
		pub fn get_platform_fee_bps() -> u32 {
			FeeConfig::<T>::get().platform_fee_bps
		}

		/// Get treasury account
		pub fn get_treasury_account() -> T::AccountId {
			FeeConfig::<T>::get().treasury_account
		}
	}
}
