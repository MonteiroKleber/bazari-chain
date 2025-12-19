#![cfg_attr(not(feature = "std"), no_std)]

//! # Bazari App Store Pallet
//!
//! This pallet manages App Store monetization on-chain for the BazariOS ecosystem.
//!
//! ## Overview
//!
//! The App Store pallet provides:
//! - App registration and metadata management
//! - App purchases with automatic revenue share
//! - In-app purchases (IAP) support
//! - Developer tier system (70-85% revenue share based on installs)
//! - Refund mechanism with configurable time window
//!
//! ## Revenue Share Tiers
//!
//! | Tier       | Installs    | Developer Share |
//! |------------|-------------|-----------------|
//! | Starter    | 0 - 1,000   | 70%             |
//! | Growth     | 1,001-10k   | 75%             |
//! | Scale      | 10,001-100k | 80%             |
//! | Enterprise | 100,001+    | 85%             |
//!
//! ## Extrinsics
//!
//! - `register_app` - Register a new app with pricing
//! - `update_app` - Update app metadata/pricing
//! - `purchase_app` - Buy an app (transfers BZR with revenue split)
//! - `purchase_iap` - Buy in-app purchase
//! - `request_refund` - Request refund within time window
//! - `process_refund` - Admin processes refund request
//! - `register_iap` - Register in-app purchase product
//! - `withdraw_revenue` - Developer withdraws accumulated revenue

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
		traits::{CheckedAdd, CheckedMul, CheckedSub, Saturating, Zero},
		Permill,
	};

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// App monetization type
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
	pub enum MonetizationType {
		#[default]
		Free,
		Paid,
		Freemium,
		Subscription,
	}

	/// In-app purchase type
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum IAPType {
		/// Can be purchased multiple times
		Consumable,
		/// One-time purchase, permanent
		NonConsumable,
		/// Recurring subscription
		Subscription,
	}

	/// Purchase status
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
	pub enum PurchaseStatus {
		#[default]
		Confirmed,
		RefundRequested,
		Refunded,
	}

	/// Refund request status
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum RefundStatus {
		Pending,
		Approved,
		Rejected,
	}

	/// Developer tier for revenue share
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum DeveloperTier {
		/// 0-1000 installs: 70% revenue share
		Starter,
		/// 1001-10000 installs: 75% revenue share
		Growth,
		/// 10001-100000 installs: 80% revenue share
		Scale,
		/// 100001+ installs: 85% revenue share
		Enterprise,
	}

	impl Default for DeveloperTier {
		fn default() -> Self {
			DeveloperTier::Starter
		}
	}

	/// App record stored on-chain
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct App<T: Config> {
		/// Unique app ID (hash of developer + slug)
		pub app_id: T::Hash,
		/// Developer account
		pub developer: T::AccountId,
		/// App slug/identifier
		pub slug: BoundedVec<u8, T::MaxSlugLength>,
		/// App name
		pub name: BoundedVec<u8, T::MaxNameLength>,
		/// Monetization type
		pub monetization_type: MonetizationType,
		/// Price in BZR (None for free apps)
		pub price: Option<BalanceOf<T>>,
		/// Total install count
		pub install_count: u64,
		/// Total revenue generated
		pub total_revenue: BalanceOf<T>,
		/// Developer's accumulated revenue (withdrawable)
		pub developer_revenue: BalanceOf<T>,
		/// Platform's accumulated revenue
		pub platform_revenue: BalanceOf<T>,
		/// Whether app is active
		pub is_active: bool,
		/// Registration block
		pub registered_at: BlockNumberFor<T>,
	}

	/// In-app purchase product
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct InAppPurchase<T: Config> {
		/// IAP ID
		pub iap_id: u64,
		/// Parent app ID
		pub app_id: T::Hash,
		/// Product ID within the app
		pub product_id: BoundedVec<u8, T::MaxSlugLength>,
		/// Product name
		pub name: BoundedVec<u8, T::MaxNameLength>,
		/// Price in BZR
		pub price: BalanceOf<T>,
		/// Purchase type
		pub iap_type: IAPType,
		/// Whether active
		pub is_active: bool,
	}

	/// Purchase record
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Purchase<T: Config> {
		/// Purchase ID
		pub purchase_id: u64,
		/// Buyer account
		pub buyer: T::AccountId,
		/// App ID
		pub app_id: T::Hash,
		/// IAP ID (None for app purchase)
		pub iap_id: Option<u64>,
		/// Amount paid
		pub amount: BalanceOf<T>,
		/// Developer share
		pub developer_share: BalanceOf<T>,
		/// Platform share
		pub platform_share: BalanceOf<T>,
		/// Status
		pub status: PurchaseStatus,
		/// Purchase block
		pub purchased_at: BlockNumberFor<T>,
	}

	/// Refund request
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct RefundRequest<T: Config> {
		/// Request ID
		pub request_id: u64,
		/// Purchase ID
		pub purchase_id: u64,
		/// Requester
		pub requester: T::AccountId,
		/// Reason
		pub reason: BoundedVec<u8, T::MaxReasonLength>,
		/// Status
		pub status: RefundStatus,
		/// Request block
		pub requested_at: BlockNumberFor<T>,
		/// Resolution block
		pub resolved_at: Option<BlockNumberFor<T>>,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Currency type for handling balances
		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		/// Maximum length for app slug
		#[pallet::constant]
		type MaxSlugLength: Get<u32>;

		/// Maximum length for app name
		#[pallet::constant]
		type MaxNameLength: Get<u32>;

		/// Maximum length for refund reason
		#[pallet::constant]
		type MaxReasonLength: Get<u32>;

		/// Refund window in blocks (e.g., 24 hours worth of blocks)
		#[pallet::constant]
		type RefundWindow: Get<BlockNumberFor<Self>>;

		/// Platform treasury account
		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		/// Origin that can process refunds
		type RefundOrigin: EnsureOrigin<Self::RuntimeOrigin>;
	}

	/// Apps storage: Hash -> App
	#[pallet::storage]
	#[pallet::getter(fn apps)]
	pub type Apps<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, App<T>>;

	/// Developer apps: AccountId -> Vec<Hash>
	#[pallet::storage]
	#[pallet::getter(fn developer_apps)]
	pub type DeveloperApps<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<T::Hash, ConstU32<100>>, ValueQuery>;

	/// In-app purchases: (AppId, IapId) -> IAP
	#[pallet::storage]
	#[pallet::getter(fn in_app_purchases)]
	pub type InAppPurchases<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::Hash, Blake2_128Concat, u64, InAppPurchase<T>>;

	/// Next IAP ID per app
	#[pallet::storage]
	#[pallet::getter(fn next_iap_id)]
	pub type NextIapId<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, u64, ValueQuery>;

	/// Purchases: PurchaseId -> Purchase
	#[pallet::storage]
	#[pallet::getter(fn purchases)]
	pub type Purchases<T: Config> = StorageMap<_, Blake2_128Concat, u64, Purchase<T>>;

	/// User app purchases: (UserId, AppId) -> PurchaseId
	#[pallet::storage]
	#[pallet::getter(fn user_app_purchases)]
	pub type UserAppPurchases<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, T::Hash, u64>;

	/// User IAP purchases: (UserId, AppId, IapId) -> Vec<PurchaseId>
	#[pallet::storage]
	#[pallet::getter(fn user_iap_purchases)]
	pub type UserIapPurchases<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, T::AccountId>,
			NMapKey<Blake2_128Concat, T::Hash>,
			NMapKey<Blake2_128Concat, u64>,
		),
		BoundedVec<u64, ConstU32<1000>>,
		ValueQuery,
	>;

	/// Next purchase ID
	#[pallet::storage]
	#[pallet::getter(fn next_purchase_id)]
	pub type NextPurchaseId<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Refund requests: RequestId -> RefundRequest
	#[pallet::storage]
	#[pallet::getter(fn refund_requests)]
	pub type RefundRequests<T: Config> = StorageMap<_, Blake2_128Concat, u64, RefundRequest<T>>;

	/// Next refund request ID
	#[pallet::storage]
	#[pallet::getter(fn next_refund_id)]
	pub type NextRefundId<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Developer tiers cache: AccountId -> DeveloperTier
	#[pallet::storage]
	#[pallet::getter(fn developer_tiers)]
	pub type DeveloperTiers<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, DeveloperTier, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// App registered
		AppRegistered {
			app_id: T::Hash,
			developer: T::AccountId,
			name: BoundedVec<u8, T::MaxNameLength>,
			monetization_type: MonetizationType,
			price: Option<BalanceOf<T>>,
		},
		/// App updated
		AppUpdated {
			app_id: T::Hash,
			monetization_type: MonetizationType,
			price: Option<BalanceOf<T>>,
		},
		/// App purchased
		AppPurchased {
			purchase_id: u64,
			app_id: T::Hash,
			buyer: T::AccountId,
			amount: BalanceOf<T>,
			developer_share: BalanceOf<T>,
			platform_share: BalanceOf<T>,
		},
		/// IAP registered
		IAPRegistered {
			app_id: T::Hash,
			iap_id: u64,
			product_id: BoundedVec<u8, T::MaxSlugLength>,
			price: BalanceOf<T>,
		},
		/// IAP purchased
		IAPPurchased {
			purchase_id: u64,
			app_id: T::Hash,
			iap_id: u64,
			buyer: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// Refund requested
		RefundRequested {
			request_id: u64,
			purchase_id: u64,
			requester: T::AccountId,
		},
		/// Refund processed
		RefundProcessed {
			request_id: u64,
			purchase_id: u64,
			approved: bool,
			amount: BalanceOf<T>,
		},
		/// Revenue withdrawn
		RevenueWithdrawn {
			developer: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// Developer tier updated
		TierUpdated {
			developer: T::AccountId,
			new_tier: DeveloperTier,
			total_installs: u64,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// App not found
		AppNotFound,
		/// App already exists
		AppAlreadyExists,
		/// Not the app developer
		NotDeveloper,
		/// App is free
		AppIsFree,
		/// Already purchased
		AlreadyPurchased,
		/// Insufficient balance
		InsufficientBalance,
		/// IAP not found
		IAPNotFound,
		/// IAP already exists
		IAPAlreadyExists,
		/// Non-consumable already purchased
		NonConsumableAlreadyPurchased,
		/// Purchase not found
		PurchaseNotFound,
		/// Refund window expired
		RefundWindowExpired,
		/// Already refunded
		AlreadyRefunded,
		/// Refund already requested
		RefundAlreadyRequested,
		/// No revenue to withdraw
		NoRevenueToWithdraw,
		/// App not active
		AppNotActive,
		/// Invalid price
		InvalidPrice,
		/// Overflow error
		Overflow,
		/// Refund request not found
		RefundRequestNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register a new app
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn register_app(
			origin: OriginFor<T>,
			slug: BoundedVec<u8, T::MaxSlugLength>,
			name: BoundedVec<u8, T::MaxNameLength>,
			monetization_type: MonetizationType,
			price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let developer = ensure_signed(origin)?;

			// Generate app ID from developer + slug
			let app_id = T::Hashing::hash_of(&(&developer, &slug));

			ensure!(!Apps::<T>::contains_key(&app_id), Error::<T>::AppAlreadyExists);

			// Validate price for paid apps
			if monetization_type == MonetizationType::Paid {
				ensure!(price.is_some() && price.unwrap() > Zero::zero(), Error::<T>::InvalidPrice);
			}

			let app = App {
				app_id,
				developer: developer.clone(),
				slug: slug.clone(),
				name: name.clone(),
				monetization_type: monetization_type.clone(),
				price,
				install_count: 0,
				total_revenue: Zero::zero(),
				developer_revenue: Zero::zero(),
				platform_revenue: Zero::zero(),
				is_active: true,
				registered_at: frame_system::Pallet::<T>::block_number(),
			};

			Apps::<T>::insert(&app_id, app);

			// Add to developer's apps
			DeveloperApps::<T>::try_mutate(&developer, |apps| -> DispatchResult {
				apps.try_push(app_id).map_err(|_| Error::<T>::Overflow)?;
				Ok(())
			})?;

			Self::deposit_event(Event::AppRegistered {
				app_id,
				developer,
				name,
				monetization_type,
				price,
			});

			Ok(())
		}

		/// Update app metadata
		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn update_app(
			origin: OriginFor<T>,
			app_id: T::Hash,
			monetization_type: MonetizationType,
			price: Option<BalanceOf<T>>,
			is_active: bool,
		) -> DispatchResult {
			let developer = ensure_signed(origin)?;

			Apps::<T>::try_mutate(&app_id, |maybe_app| -> DispatchResult {
				let app = maybe_app.as_mut().ok_or(Error::<T>::AppNotFound)?;
				ensure!(app.developer == developer, Error::<T>::NotDeveloper);

				if monetization_type == MonetizationType::Paid {
					ensure!(price.is_some() && price.unwrap() > Zero::zero(), Error::<T>::InvalidPrice);
				}

				app.monetization_type = monetization_type.clone();
				app.price = price;
				app.is_active = is_active;

				Self::deposit_event(Event::AppUpdated {
					app_id,
					monetization_type,
					price,
				});

				Ok(())
			})
		}

		/// Purchase an app
		#[pallet::call_index(2)]
		#[pallet::weight(10_000)]
		pub fn purchase_app(origin: OriginFor<T>, app_id: T::Hash) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			let app = Apps::<T>::get(&app_id).ok_or(Error::<T>::AppNotFound)?;
			ensure!(app.is_active, Error::<T>::AppNotActive);
			ensure!(app.monetization_type != MonetizationType::Free, Error::<T>::AppIsFree);

			// Check not already purchased
			ensure!(
				!UserAppPurchases::<T>::contains_key(&buyer, &app_id),
				Error::<T>::AlreadyPurchased
			);

			let price = app.price.ok_or(Error::<T>::InvalidPrice)?;

			// Calculate revenue share based on developer tier
			let tier = Self::get_developer_tier(&app.developer);
			let (developer_share, platform_share) = Self::calculate_revenue_share(price, &tier)?;

			// Transfer funds
			T::Currency::transfer(
				&buyer,
				&app.developer,
				developer_share,
				ExistenceRequirement::KeepAlive,
			)?;
			T::Currency::transfer(
				&buyer,
				&T::TreasuryAccount::get(),
				platform_share,
				ExistenceRequirement::KeepAlive,
			)?;

			// Create purchase record
			let purchase_id = NextPurchaseId::<T>::get();
			NextPurchaseId::<T>::put(purchase_id.saturating_add(1));

			let purchase = Purchase {
				purchase_id,
				buyer: buyer.clone(),
				app_id,
				iap_id: None,
				amount: price,
				developer_share,
				platform_share,
				status: PurchaseStatus::Confirmed,
				purchased_at: frame_system::Pallet::<T>::block_number(),
			};

			Purchases::<T>::insert(purchase_id, purchase);
			UserAppPurchases::<T>::insert(&buyer, &app_id, purchase_id);

			// Update app stats
			Apps::<T>::try_mutate(&app_id, |maybe_app| -> DispatchResult {
				let app = maybe_app.as_mut().ok_or(Error::<T>::AppNotFound)?;
				app.install_count = app.install_count.saturating_add(1);
				app.total_revenue = app.total_revenue.saturating_add(price);
				app.developer_revenue = app.developer_revenue.saturating_add(developer_share);
				app.platform_revenue = app.platform_revenue.saturating_add(platform_share);
				Ok(())
			})?;

			// Update developer tier if needed
			Self::update_developer_tier(&app.developer)?;

			Self::deposit_event(Event::AppPurchased {
				purchase_id,
				app_id,
				buyer,
				amount: price,
				developer_share,
				platform_share,
			});

			Ok(())
		}

		/// Register an in-app purchase product
		#[pallet::call_index(3)]
		#[pallet::weight(10_000)]
		pub fn register_iap(
			origin: OriginFor<T>,
			app_id: T::Hash,
			product_id: BoundedVec<u8, T::MaxSlugLength>,
			name: BoundedVec<u8, T::MaxNameLength>,
			price: BalanceOf<T>,
			iap_type: IAPType,
		) -> DispatchResult {
			let developer = ensure_signed(origin)?;

			let app = Apps::<T>::get(&app_id).ok_or(Error::<T>::AppNotFound)?;
			ensure!(app.developer == developer, Error::<T>::NotDeveloper);
			ensure!(price > Zero::zero(), Error::<T>::InvalidPrice);

			let iap_id = NextIapId::<T>::get(&app_id);

			// Check product_id uniqueness within app
			for i in 0..iap_id {
				if let Some(existing) = InAppPurchases::<T>::get(&app_id, i) {
					ensure!(existing.product_id != product_id, Error::<T>::IAPAlreadyExists);
				}
			}

			let iap = InAppPurchase {
				iap_id,
				app_id,
				product_id: product_id.clone(),
				name,
				price,
				iap_type,
				is_active: true,
			};

			InAppPurchases::<T>::insert(&app_id, iap_id, iap);
			NextIapId::<T>::put(&app_id, iap_id.saturating_add(1));

			Self::deposit_event(Event::IAPRegistered {
				app_id,
				iap_id,
				product_id,
				price,
			});

			Ok(())
		}

		/// Purchase an in-app product
		#[pallet::call_index(4)]
		#[pallet::weight(10_000)]
		pub fn purchase_iap(
			origin: OriginFor<T>,
			app_id: T::Hash,
			iap_id: u64,
		) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			let app = Apps::<T>::get(&app_id).ok_or(Error::<T>::AppNotFound)?;
			ensure!(app.is_active, Error::<T>::AppNotActive);

			let iap = InAppPurchases::<T>::get(&app_id, iap_id).ok_or(Error::<T>::IAPNotFound)?;
			ensure!(iap.is_active, Error::<T>::AppNotActive);

			// For non-consumable, check not already purchased
			if iap.iap_type == IAPType::NonConsumable {
				let user_purchases = UserIapPurchases::<T>::get((&buyer, &app_id, iap_id));
				ensure!(user_purchases.is_empty(), Error::<T>::NonConsumableAlreadyPurchased);
			}

			let price = iap.price;

			// Calculate revenue share
			let tier = Self::get_developer_tier(&app.developer);
			let (developer_share, platform_share) = Self::calculate_revenue_share(price, &tier)?;

			// Transfer funds
			T::Currency::transfer(
				&buyer,
				&app.developer,
				developer_share,
				ExistenceRequirement::KeepAlive,
			)?;
			T::Currency::transfer(
				&buyer,
				&T::TreasuryAccount::get(),
				platform_share,
				ExistenceRequirement::KeepAlive,
			)?;

			// Create purchase record
			let purchase_id = NextPurchaseId::<T>::get();
			NextPurchaseId::<T>::put(purchase_id.saturating_add(1));

			let purchase = Purchase {
				purchase_id,
				buyer: buyer.clone(),
				app_id,
				iap_id: Some(iap_id),
				amount: price,
				developer_share,
				platform_share,
				status: PurchaseStatus::Confirmed,
				purchased_at: frame_system::Pallet::<T>::block_number(),
			};

			Purchases::<T>::insert(purchase_id, purchase);

			// Track user's IAP purchases
			UserIapPurchases::<T>::try_mutate(
				(&buyer, &app_id, iap_id),
				|purchases| -> DispatchResult {
					purchases.try_push(purchase_id).map_err(|_| Error::<T>::Overflow)?;
					Ok(())
				},
			)?;

			// Update app revenue
			Apps::<T>::try_mutate(&app_id, |maybe_app| -> DispatchResult {
				let app = maybe_app.as_mut().ok_or(Error::<T>::AppNotFound)?;
				app.total_revenue = app.total_revenue.saturating_add(price);
				app.developer_revenue = app.developer_revenue.saturating_add(developer_share);
				app.platform_revenue = app.platform_revenue.saturating_add(platform_share);
				Ok(())
			})?;

			Self::deposit_event(Event::IAPPurchased {
				purchase_id,
				app_id,
				iap_id,
				buyer,
				amount: price,
			});

			Ok(())
		}

		/// Request a refund
		#[pallet::call_index(5)]
		#[pallet::weight(10_000)]
		pub fn request_refund(
			origin: OriginFor<T>,
			purchase_id: u64,
			reason: BoundedVec<u8, T::MaxReasonLength>,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;

			let purchase = Purchases::<T>::get(purchase_id).ok_or(Error::<T>::PurchaseNotFound)?;
			ensure!(purchase.buyer == requester, Error::<T>::NotDeveloper);
			ensure!(purchase.status == PurchaseStatus::Confirmed, Error::<T>::AlreadyRefunded);

			// Check refund window
			let current_block = frame_system::Pallet::<T>::block_number();
			let refund_deadline = purchase.purchased_at.saturating_add(T::RefundWindow::get());
			ensure!(current_block <= refund_deadline, Error::<T>::RefundWindowExpired);

			let request_id = NextRefundId::<T>::get();
			NextRefundId::<T>::put(request_id.saturating_add(1));

			let request = RefundRequest {
				request_id,
				purchase_id,
				requester: requester.clone(),
				reason,
				status: RefundStatus::Pending,
				requested_at: current_block,
				resolved_at: None,
			};

			RefundRequests::<T>::insert(request_id, request);

			// Mark purchase as refund requested
			Purchases::<T>::try_mutate(purchase_id, |maybe_purchase| -> DispatchResult {
				let purchase = maybe_purchase.as_mut().ok_or(Error::<T>::PurchaseNotFound)?;
				purchase.status = PurchaseStatus::RefundRequested;
				Ok(())
			})?;

			Self::deposit_event(Event::RefundRequested {
				request_id,
				purchase_id,
				requester,
			});

			Ok(())
		}

		/// Process a refund request (admin only)
		#[pallet::call_index(6)]
		#[pallet::weight(10_000)]
		pub fn process_refund(
			origin: OriginFor<T>,
			request_id: u64,
			approve: bool,
		) -> DispatchResult {
			T::RefundOrigin::ensure_origin(origin)?;

			let request =
				RefundRequests::<T>::get(request_id).ok_or(Error::<T>::RefundRequestNotFound)?;
			ensure!(request.status == RefundStatus::Pending, Error::<T>::AlreadyRefunded);

			let purchase =
				Purchases::<T>::get(request.purchase_id).ok_or(Error::<T>::PurchaseNotFound)?;
			let app = Apps::<T>::get(&purchase.app_id).ok_or(Error::<T>::AppNotFound)?;

			let mut refund_amount = Zero::zero();

			if approve {
				// Refund from developer and treasury
				T::Currency::transfer(
					&app.developer,
					&purchase.buyer,
					purchase.developer_share,
					ExistenceRequirement::AllowDeath,
				)?;
				T::Currency::transfer(
					&T::TreasuryAccount::get(),
					&purchase.buyer,
					purchase.platform_share,
					ExistenceRequirement::AllowDeath,
				)?;

				refund_amount = purchase.amount;

				// Update purchase status
				Purchases::<T>::try_mutate(request.purchase_id, |maybe_purchase| -> DispatchResult {
					let purchase = maybe_purchase.as_mut().ok_or(Error::<T>::PurchaseNotFound)?;
					purchase.status = PurchaseStatus::Refunded;
					Ok(())
				})?;

				// Update app stats
				Apps::<T>::try_mutate(&purchase.app_id, |maybe_app| -> DispatchResult {
					let app = maybe_app.as_mut().ok_or(Error::<T>::AppNotFound)?;
					app.install_count = app.install_count.saturating_sub(1);
					app.total_revenue = app.total_revenue.saturating_sub(purchase.amount);
					app.developer_revenue = app.developer_revenue.saturating_sub(purchase.developer_share);
					app.platform_revenue = app.platform_revenue.saturating_sub(purchase.platform_share);
					Ok(())
				})?;

				// Remove from user purchases if app purchase
				if purchase.iap_id.is_none() {
					UserAppPurchases::<T>::remove(&purchase.buyer, &purchase.app_id);
				}
			} else {
				// Restore purchase status
				Purchases::<T>::try_mutate(request.purchase_id, |maybe_purchase| -> DispatchResult {
					let purchase = maybe_purchase.as_mut().ok_or(Error::<T>::PurchaseNotFound)?;
					purchase.status = PurchaseStatus::Confirmed;
					Ok(())
				})?;
			}

			// Update request status
			RefundRequests::<T>::try_mutate(request_id, |maybe_request| -> DispatchResult {
				let request = maybe_request.as_mut().ok_or(Error::<T>::RefundRequestNotFound)?;
				request.status = if approve {
					RefundStatus::Approved
				} else {
					RefundStatus::Rejected
				};
				request.resolved_at = Some(frame_system::Pallet::<T>::block_number());
				Ok(())
			})?;

			Self::deposit_event(Event::RefundProcessed {
				request_id,
				purchase_id: request.purchase_id,
				approved: approve,
				amount: refund_amount,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get developer tier based on total installs
		pub fn get_developer_tier(developer: &T::AccountId) -> DeveloperTier {
			DeveloperTiers::<T>::get(developer)
		}

		/// Calculate revenue share based on tier
		pub fn calculate_revenue_share(
			amount: BalanceOf<T>,
			tier: &DeveloperTier,
		) -> Result<(BalanceOf<T>, BalanceOf<T>), DispatchError> {
			let developer_percentage = match tier {
				DeveloperTier::Starter => Permill::from_percent(70),
				DeveloperTier::Growth => Permill::from_percent(75),
				DeveloperTier::Scale => Permill::from_percent(80),
				DeveloperTier::Enterprise => Permill::from_percent(85),
			};

			let developer_share = developer_percentage.mul_floor(amount);
			let platform_share = amount.saturating_sub(developer_share);

			Ok((developer_share, platform_share))
		}

		/// Update developer tier based on total installs
		pub fn update_developer_tier(developer: &T::AccountId) -> DispatchResult {
			let apps = DeveloperApps::<T>::get(developer);
			let mut total_installs: u64 = 0;

			for app_id in apps.iter() {
				if let Some(app) = Apps::<T>::get(app_id) {
					total_installs = total_installs.saturating_add(app.install_count);
				}
			}

			let new_tier = if total_installs > 100_000 {
				DeveloperTier::Enterprise
			} else if total_installs > 10_000 {
				DeveloperTier::Scale
			} else if total_installs > 1_000 {
				DeveloperTier::Growth
			} else {
				DeveloperTier::Starter
			};

			let current_tier = DeveloperTiers::<T>::get(developer);

			if new_tier != current_tier {
				DeveloperTiers::<T>::insert(developer, new_tier.clone());

				Self::deposit_event(Event::TierUpdated {
					developer: developer.clone(),
					new_tier,
					total_installs,
				});
			}

			Ok(())
		}

		/// Check if user has purchased an app
		pub fn has_purchased_app(user: &T::AccountId, app_id: &T::Hash) -> bool {
			UserAppPurchases::<T>::contains_key(user, app_id)
		}

		/// Check if user has purchased an IAP
		pub fn has_purchased_iap(user: &T::AccountId, app_id: &T::Hash, iap_id: u64) -> bool {
			let purchases = UserIapPurchases::<T>::get((user, app_id, iap_id));
			!purchases.is_empty()
		}
	}
}
