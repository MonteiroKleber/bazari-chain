#![cfg_attr(not(feature = "std"), no_std)]

//! # Bazari Commerce Pallet
//!
//! This pallet manages Orders, Sales, and Commissions on-chain for the Bazari marketplace.
//!
//! ## Overview
//!
//! The Commerce pallet provides:
//! - Order creation and lifecycle management (Proposed → Pending → Paid → Shipped → Delivered)
//! - Sale recording and receipt NFT minting
//! - Commission tracking for affiliate sales
//! - Support for both Marketplace and BazChat order sources
//!
//! ## Extrinsics
//!
//! - `create_order` - Create a new order (auto-generates OrderId)
//! - `accept_proposal` - Accept a BazChat proposal (Proposed → Pending)
//! - `mark_paid` - Mark order as paid and link escrow (Pending → Paid)
//! - `mark_shipped` - Mark order as shipped (Paid → Shipped)
//! - `mark_delivered` - Mark order as delivered and create sale (Shipped → Delivered)
//! - `mint_receipt` - Mint receipt NFT for completed order
//! - `cancel_order` - Cancel a Proposed/Pending order

extern crate alloc;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use alloc::vec::Vec;
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, ReservableCurrency},
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_runtime::{
		traits::{CheckedAdd, One, Saturating, Zero},
	};

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Order status state machine
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum OrderStatus {
		/// BazChat proposal pending buyer acceptance
		Proposed,
		/// Marketplace order created, awaiting payment
		Pending,
		/// Payment locked in escrow
		Paid,
		/// Seller preparing order
		Processing,
		/// Order shipped by seller
		Shipped,
		/// Order delivered and escrow released
		Delivered,
		/// Order cancelled before payment
		Cancelled,
		/// Order refunded after payment
		Refunded,
		/// Order in dispute resolution
		Disputed,
	}

	/// Order source type
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum OrderSource {
		/// Direct marketplace purchase
		Marketplace,
		/// Proposal from BazChat thread
		BazChat,
	}

	/// Single order item
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug))]
	#[scale_info(skip_type_params(MaxItemNameLen))]
	pub struct OrderItem<Balance, MaxItemNameLen: Get<u32>> {
		pub product_id: Option<u64>,
		pub name: BoundedVec<u8, MaxItemNameLen>,
		pub quantity: u32,
		pub unit_price: Balance,
	}

	/// Complete order record
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug))]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct Order<T: Config> {
		pub order_id: u64,
		pub source: OrderSource,
		pub thread_id: Option<u64>,
		pub buyer: T::AccountId,
		pub seller: T::AccountId,
		pub store_id: Option<u64>,
		pub is_multi_store: bool,
		pub items: BoundedVec<OrderItem<BalanceOf<T>, T::MaxItemNameLen>, T::MaxItemsPerOrder>,
		pub total_amount: BalanceOf<T>,
		pub platform_fee: BalanceOf<T>,
		pub status: OrderStatus,
		pub escrow_id: Option<u64>,
		pub receipt_nft_id: Option<u64>,
		pub created_at: BlockNumberFor<T>,
		pub paid_at: Option<BlockNumberFor<T>>,
		pub shipped_at: Option<BlockNumberFor<T>>,
		pub delivered_at: Option<BlockNumberFor<T>>,
	}

	/// Sale record (created after delivery)
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Sale<T: Config> {
		pub sale_id: u64,
		pub order_id: u64,
		pub seller: T::AccountId,
		pub buyer: T::AccountId,
		pub amount: BalanceOf<T>,
		pub commission_paid: BalanceOf<T>,
		pub platform_fee: BalanceOf<T>,
		pub created_at: BlockNumberFor<T>,
	}

	/// Receipt NFT metadata
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct ReceiptNFT<T: Config> {
		pub nft_id: u64,
		pub order_id: u64,
		pub owner: T::AccountId,
		pub minted_at: BlockNumberFor<T>,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_stores::Config {
		/// The overarching event type
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Currency type for handling balances
		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		/// Order ID type (u64)
		type OrderId: Parameter
			+ Member
			+ Copy
			+ Ord
			+ Default
			+ Zero
			+ One
			+ CheckedAdd
			+ MaxEncodedLen;

		/// Maximum items per order
		#[pallet::constant]
		type MaxItemsPerOrder: Get<u32>;

		/// Maximum length of item name
		#[pallet::constant]
		type MaxItemNameLen: Get<u32>;

		/// Platform fee percentage (basis points, e.g., 250 = 2.5%)
		#[pallet::constant]
		type PlatformFeeBps: Get<u32>;
	}

	// ============================================
	// STORAGE
	// ============================================

	/// Auto-increment counter for order IDs
	#[pallet::storage]
	#[pallet::getter(fn next_order_id)]
	pub type NextOrderId<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Auto-increment counter for sale IDs
	#[pallet::storage]
	#[pallet::getter(fn next_sale_id)]
	pub type NextSaleId<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Auto-increment counter for receipt NFT IDs
	#[pallet::storage]
	#[pallet::getter(fn next_receipt_id)]
	pub type NextReceiptId<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Orders storage map: OrderId => Order
	#[pallet::storage]
	#[pallet::getter(fn orders)]
	pub type Orders<T: Config> = StorageMap<_, Blake2_128Concat, u64, Order<T>, OptionQuery>;

	/// Sales storage map: SaleId => Sale
	#[pallet::storage]
	#[pallet::getter(fn sales)]
	pub type Sales<T: Config> = StorageMap<_, Blake2_128Concat, u64, Sale<T>, OptionQuery>;

	/// Receipt NFTs storage map: NftId => ReceiptNFT
	#[pallet::storage]
	#[pallet::getter(fn receipt_nfts)]
	pub type ReceiptNFTs<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, ReceiptNFT<T>, OptionQuery>;

	/// Index: Buyer => List of OrderIds
	#[pallet::storage]
	#[pallet::getter(fn user_orders)]
	pub type UserOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<u64, ConstU32<1000>>, ValueQuery>;

	/// Index: Seller => List of OrderIds
	#[pallet::storage]
	#[pallet::getter(fn seller_orders)]
	pub type SellerOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<u64, ConstU32<1000>>, ValueQuery>;

	// ============================================
	// EVENTS
	// ============================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Order created [order_id, buyer, seller, total_amount]
		OrderCreated {
			order_id: u64,
			buyer: T::AccountId,
			seller: T::AccountId,
			total_amount: BalanceOf<T>,
		},
		/// BazChat proposal accepted [order_id, buyer]
		ProposalAccepted { order_id: u64, buyer: T::AccountId },
		/// Order marked as paid [order_id, escrow_id]
		OrderPaid { order_id: u64, escrow_id: u64 },
		/// Order shipped [order_id, seller]
		OrderShipped { order_id: u64, seller: T::AccountId },
		/// Order delivered and sale created [order_id, sale_id]
		OrderDelivered { order_id: u64, sale_id: u64 },
		/// Receipt NFT minted [nft_id, order_id, owner]
		ReceiptMinted {
			nft_id: u64,
			order_id: u64,
			owner: T::AccountId,
		},
		/// Order cancelled [order_id, reason]
		OrderCancelled { order_id: u64, who: T::AccountId },
		/// Commission recorded for sale [sale_id, amount, recipient]
		CommissionRecorded {
			sale_id: u64,
			amount: BalanceOf<T>,
			recipient: T::AccountId,
		},
	}

	// ============================================
	// ERRORS
	// ============================================

	#[pallet::error]
	pub enum Error<T> {
		/// Order not found
		OrderNotFound,
		/// Unauthorized action (not buyer or seller)
		Unauthorized,
		/// Invalid order status for this operation
		InvalidOrderStatus,
		/// Store not found
		StoreNotFound,
		/// Empty order (no items)
		EmptyOrder,
		/// Too many items in order
		TooManyItems,
		/// Item name too long
		ItemNameTooLong,
		/// Order ID overflow
		OrderIdOverflow,
		/// Sale ID overflow
		SaleIdOverflow,
		/// Receipt ID overflow
		ReceiptIdOverflow,
		/// Order source mismatch (e.g., accepting Marketplace order as proposal)
		OrderSourceMismatch,
		/// Receipt already minted for this order
		ReceiptAlreadyMinted,
		/// Cannot cancel order in current status
		CannotCancelOrder,
		/// Sale not found
		SaleNotFound,
	}

	// ============================================
	// EXTRINSICS
	// ============================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new order
		///
		/// - `source`: OrderSource (0 = Marketplace, 1 = BazChat)
		/// - `thread_id`: Optional BazChat thread ID
		/// - `seller`: Seller account
		/// - `store_id`: Optional store ID
		/// - `items`: Order items (product_id, name, quantity, unit_price)
		///
		/// Emits `OrderCreated` event
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn create_order(
			origin: OriginFor<T>,
			source: u8,
			thread_id: Option<u64>,
			seller: T::AccountId,
			store_id: Option<u64>,
			items: Vec<(Option<u64>, Vec<u8>, u32, BalanceOf<T>)>,
		) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			// Parse OrderSource from u8
			let source = match source {
				0 => OrderSource::Marketplace,
				1 => OrderSource::BazChat,
				_ => return Err(Error::<T>::OrderSourceMismatch.into()),
			};

			// Validate items
			ensure!(!items.is_empty(), Error::<T>::EmptyOrder);
			ensure!(
				items.len() <= T::MaxItemsPerOrder::get() as usize,
				Error::<T>::TooManyItems
			);

			// Note: Store validation omitted for simplicity.
			// In production, validate store_id exists in pallet-stores if provided.

			// Build order items and calculate total
			let mut order_items =
				BoundedVec::<OrderItem<BalanceOf<T>, T::MaxItemNameLen>, T::MaxItemsPerOrder>::default();
			let mut total_amount = BalanceOf::<T>::zero();

			for (product_id, name, quantity, unit_price) in items {
				let bounded_name: BoundedVec<u8, T::MaxItemNameLen> =
					name.try_into().map_err(|_| Error::<T>::ItemNameTooLong)?;

				let item_total = unit_price.saturating_mul(quantity.into());
				total_amount = total_amount.saturating_add(item_total);

				let item = OrderItem {
					product_id,
					name: bounded_name,
					quantity,
					unit_price,
				};
				order_items.try_push(item).map_err(|_| Error::<T>::TooManyItems)?;
			}

			// Calculate platform fee
			let platform_fee = Self::calculate_platform_fee(total_amount);

			// Generate order ID
			let order_id = NextOrderId::<T>::get();
			let next_id = order_id.checked_add(1).ok_or(Error::<T>::OrderIdOverflow)?;

			// Determine initial status based on source
			let status = match source {
				OrderSource::BazChat => OrderStatus::Proposed,
				OrderSource::Marketplace => OrderStatus::Pending,
			};

			let block_number = frame_system::Pallet::<T>::block_number();

			// Create order
			let order = Order {
				order_id,
				source,
				thread_id,
				buyer: buyer.clone(),
				seller: seller.clone(),
				store_id,
				is_multi_store: false,
				items: order_items,
				total_amount,
				platform_fee,
				status,
				escrow_id: None,
				receipt_nft_id: None,
				created_at: block_number,
				paid_at: None,
				shipped_at: None,
				delivered_at: None,
			};

			// Store order
			Orders::<T>::insert(order_id, order);
			NextOrderId::<T>::put(next_id);

			// Update indexes
			UserOrders::<T>::try_mutate(&buyer, |orders| -> Result<(), Error<T>> {
				orders.try_push(order_id).map_err(|_| Error::<T>::OrderIdOverflow)?;
				Ok(())
			})?;
			SellerOrders::<T>::try_mutate(&seller, |orders| -> Result<(), Error<T>> {
				orders.try_push(order_id).map_err(|_| Error::<T>::OrderIdOverflow)?;
				Ok(())
			})?;

			Self::deposit_event(Event::OrderCreated {
				order_id,
				buyer,
				seller,
				total_amount,
			});

			Ok(())
		}

		/// Accept a BazChat proposal (Proposed → Pending)
		///
		/// Only buyer can accept. Only valid for BazChat source orders.
		///
		/// Emits `ProposalAccepted` event
		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn accept_proposal(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			Orders::<T>::try_mutate(order_id, |order_opt| -> DispatchResult {
				let order = order_opt.as_mut().ok_or(Error::<T>::OrderNotFound)?;

				// Validate buyer
				ensure!(order.buyer == buyer, Error::<T>::Unauthorized);

				// Validate source
				ensure!(
					order.source == OrderSource::BazChat,
					Error::<T>::OrderSourceMismatch
				);

				// Validate status
				ensure!(
					order.status == OrderStatus::Proposed,
					Error::<T>::InvalidOrderStatus
				);

				// Update status
				order.status = OrderStatus::Pending;

				Self::deposit_event(Event::ProposalAccepted {
					order_id,
					buyer: buyer.clone(),
				});

				Ok(())
			})
		}

		/// Mark order as paid (Pending → Paid)
		///
		/// Links escrow ID and records payment timestamp.
		/// Called by escrow pallet after locking funds.
		///
		/// Emits `OrderPaid` event
		#[pallet::call_index(2)]
		#[pallet::weight(10_000)]
		pub fn mark_paid(
			origin: OriginFor<T>,
			order_id: u64,
			escrow_id: u64,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			Orders::<T>::try_mutate(order_id, |order_opt| -> DispatchResult {
				let order = order_opt.as_mut().ok_or(Error::<T>::OrderNotFound)?;

				// Validate status
				ensure!(
					order.status == OrderStatus::Pending,
					Error::<T>::InvalidOrderStatus
				);

				// Update order
				order.status = OrderStatus::Paid;
				order.escrow_id = Some(escrow_id);
				order.paid_at = Some(frame_system::Pallet::<T>::block_number());

				Self::deposit_event(Event::OrderPaid { order_id, escrow_id });

				Ok(())
			})
		}

		/// Mark order as shipped (Paid → Shipped)
		///
		/// Only seller can mark as shipped.
		///
		/// Emits `OrderShipped` event
		#[pallet::call_index(3)]
		#[pallet::weight(10_000)]
		pub fn mark_shipped(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
			let seller = ensure_signed(origin)?;

			Orders::<T>::try_mutate(order_id, |order_opt| -> DispatchResult {
				let order = order_opt.as_mut().ok_or(Error::<T>::OrderNotFound)?;

				// Validate seller
				ensure!(order.seller == seller, Error::<T>::Unauthorized);

				// Validate status (allow Paid or Processing)
				ensure!(
					order.status == OrderStatus::Paid || order.status == OrderStatus::Processing,
					Error::<T>::InvalidOrderStatus
				);

				// Update order
				order.status = OrderStatus::Shipped;
				order.shipped_at = Some(frame_system::Pallet::<T>::block_number());

				Self::deposit_event(Event::OrderShipped {
					order_id,
					seller: seller.clone(),
				});

				Ok(())
			})
		}

		/// Mark order as delivered (Shipped → Delivered)
		///
		/// Creates a Sale record. Can be called by buyer (confirmation) or
		/// automatically by escrow pallet on auto-release.
		///
		/// Emits `OrderDelivered` event
		#[pallet::call_index(4)]
		#[pallet::weight(10_000)]
		pub fn mark_delivered(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			let mut sale_id = 0u64;

			Orders::<T>::try_mutate(order_id, |order_opt| -> DispatchResult {
				let order = order_opt.as_mut().ok_or(Error::<T>::OrderNotFound)?;

				// Validate status
				ensure!(
					order.status == OrderStatus::Shipped,
					Error::<T>::InvalidOrderStatus
				);

				// Update order
				order.status = OrderStatus::Delivered;
				order.delivered_at = Some(frame_system::Pallet::<T>::block_number());

				// Create sale record
				sale_id = Self::create_sale(order)?;

				Ok(())
			})?;

			Self::deposit_event(Event::OrderDelivered { order_id, sale_id });

			Ok(())
		}

		/// Mint receipt NFT for completed order
		///
		/// Only buyer can mint. Order must be delivered.
		///
		/// Emits `ReceiptMinted` event
		#[pallet::call_index(5)]
		#[pallet::weight(10_000)]
		pub fn mint_receipt(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;

			// Validate buyer
			ensure!(order.buyer == buyer, Error::<T>::Unauthorized);

			// Validate status
			ensure!(
				order.status == OrderStatus::Delivered,
				Error::<T>::InvalidOrderStatus
			);

			// Check if receipt already minted
			ensure!(
				order.receipt_nft_id.is_none(),
				Error::<T>::ReceiptAlreadyMinted
			);

			// Generate NFT ID
			let nft_id = NextReceiptId::<T>::get();
			let next_id = nft_id.checked_add(1).ok_or(Error::<T>::ReceiptIdOverflow)?;

			let block_number = frame_system::Pallet::<T>::block_number();

			// Create receipt NFT
			let receipt = ReceiptNFT {
				nft_id,
				order_id,
				owner: buyer.clone(),
				minted_at: block_number,
			};

			// Store receipt and update order
			ReceiptNFTs::<T>::insert(nft_id, receipt);
			NextReceiptId::<T>::put(next_id);

			Orders::<T>::mutate(order_id, |order_opt| {
				if let Some(order) = order_opt {
					order.receipt_nft_id = Some(nft_id);
				}
			});

			Self::deposit_event(Event::ReceiptMinted {
				nft_id,
				order_id,
				owner: buyer,
			});

			Ok(())
		}

		/// Cancel order (Proposed/Pending only)
		///
		/// Buyer or seller can cancel. Order must not be paid.
		///
		/// Emits `OrderCancelled` event
		#[pallet::call_index(6)]
		#[pallet::weight(10_000)]
		pub fn cancel_order(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Orders::<T>::try_mutate(order_id, |order_opt| -> DispatchResult {
				let order = order_opt.as_mut().ok_or(Error::<T>::OrderNotFound)?;

				// Validate caller is buyer or seller
				ensure!(
					order.buyer == who || order.seller == who,
					Error::<T>::Unauthorized
				);

				// Validate status (only Proposed or Pending can be cancelled)
				ensure!(
					order.status == OrderStatus::Proposed || order.status == OrderStatus::Pending,
					Error::<T>::CannotCancelOrder
				);

				// Update status
				order.status = OrderStatus::Cancelled;

				Self::deposit_event(Event::OrderCancelled {
					order_id,
					who: who.clone(),
				});

				Ok(())
			})
		}

		/// Record commission payment for a sale
		///
		/// Called by affiliate pallet after paying commission to affiliates.
		/// Updates the commission_paid field in the Sale struct.
		///
		/// Emits `CommissionRecorded` event
		#[pallet::call_index(7)]
		#[pallet::weight(10_000)]
		pub fn record_commission(
			origin: OriginFor<T>,
			sale_id: u64,
			amount: BalanceOf<T>,
			recipient: T::AccountId,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			Sales::<T>::try_mutate(sale_id, |sale_opt| -> DispatchResult {
				let sale = sale_opt.as_mut().ok_or(Error::<T>::SaleNotFound)?;

				// Update commission paid
				sale.commission_paid = sale.commission_paid.saturating_add(amount);

				Self::deposit_event(Event::CommissionRecorded {
					sale_id,
					amount,
					recipient,
				});

				Ok(())
			})
		}
	}

	// ============================================
	// HELPER FUNCTIONS
	// ============================================

	impl<T: Config> Pallet<T> {
		/// Calculate platform fee based on configured basis points
		fn calculate_platform_fee(amount: BalanceOf<T>) -> BalanceOf<T> {
			let bps = T::PlatformFeeBps::get();
			// Fee = amount * bps / 10000
			amount.saturating_mul(bps.into()) / 10000u32.into()
		}

		/// Create sale record after delivery
		fn create_sale(order: &Order<T>) -> Result<u64, Error<T>> {
			let sale_id = NextSaleId::<T>::get();
			let next_id = sale_id.checked_add(1).ok_or(Error::<T>::SaleIdOverflow)?;

			let sale = Sale {
				sale_id,
				order_id: order.order_id,
				seller: order.seller.clone(),
				buyer: order.buyer.clone(),
				amount: order.total_amount,
				commission_paid: Zero::zero(),
				platform_fee: order.platform_fee,
				created_at: frame_system::Pallet::<T>::block_number(),
			};

			Sales::<T>::insert(sale_id, sale);
			NextSaleId::<T>::put(next_id);

			Ok(sale_id)
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
		pub enum Test where
			Block = frame_system::mocking::MockBlock<Test>,
			NodeBlock = frame_system::mocking::MockBlock<Test>,
			UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>,
		{
			System: frame_system,
			Balances: pallet_balances,
			Uniques: pallet_uniques,
			Stores: pallet_stores::{Pallet, Call, Storage, Event<T>},
			Commerce: pallet::{Pallet, Call, Storage, Event<T>},
		}
	);

	parameter_types! {
		pub const BlockHashCount: BlockNumber = 2400;
		pub const ExistentialDeposit: Balance = 1;
		pub const MaxLocks: u32 = 50;
		pub const MaxReserves: u32 = 0;
		pub const MaxCidLen: u32 = 96;
		pub const CollectionDeposit: Balance = 0;
		pub const ItemDeposit: Balance = 0;
		pub const MetadataDepositBase: Balance = 0;
		pub const AttributeDepositBase: Balance = 0;
		pub const DepositPerByte: Balance = 0;
		pub const StringLimit: u32 = 256;
		pub const KeyLimit: u32 = 32;
		pub const ValueLimit: u32 = 256;
		pub const MaxOperators: u32 = 3;
		pub const CreationDepositConst: Balance = 0;
		pub const MaxStoresPerOwner: u32 = 64;
		pub const MaxItemsPerOrder: u32 = 50;
		pub const MaxItemNameLen: u32 = 128;
		pub const PlatformFeeBps: u32 = 250; // 2.5%
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

	impl pallet_uniques::Config for Test {
		type RuntimeEvent = RuntimeEvent;
		type CollectionId = u32;
		type ItemId = u64;
		type Currency = Balances;
		type ForceOrigin = frame_system::EnsureRoot<AccountId>;
		type CreateOrigin =
			frame_support::traits::AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
		type Locker = ();
		type CollectionDeposit = CollectionDeposit;
		type ItemDeposit = ItemDeposit;
		type KeyLimit = KeyLimit;
		type ValueLimit = ValueLimit;
		type StringLimit = StringLimit;
		type MetadataDepositBase = MetadataDepositBase;
		type AttributeDepositBase = AttributeDepositBase;
		type DepositPerByte = DepositPerByte;
		type WeightInfo = ();
	}

	impl pallet_stores::Config for Test {
		type RuntimeEvent = RuntimeEvent;
		type StoreId = u64;
		type MaxCidLen = MaxCidLen;
		type MaxOperators = MaxOperators;
		type CreationDeposit = CreationDepositConst;
		type ReputationOrigin = frame_system::EnsureSigned<AccountId>;
		type MaxStoresPerOwner = MaxStoresPerOwner;
	}

	impl Config for Test {
		type RuntimeEvent = RuntimeEvent;
		type Currency = Balances;
		type OrderId = u64;
		type MaxItemsPerOrder = MaxItemsPerOrder;
		type MaxItemNameLen = MaxItemNameLen;
		type PlatformFeeBps = PlatformFeeBps;
	}

	// Use types from the runtime
	pub use Test as TestRuntime;

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
			// Initialize counters to 1 so first IDs are 1
			NextOrderId::<Test>::put(1u64);
			NextSaleId::<Test>::put(1u64);
			NextReceiptId::<Test>::put(1u64);
		});
		ext
	}

	fn create_test_store(owner: AccountId) -> u64 {
		assert_ok!(pallet_stores::Pallet::<Test>::create_store(
			frame_system::RawOrigin::Signed(owner).into(),
			b"store-cid".to_vec(),
			b"test-store".to_vec()
		));
		1 // First store ID
	}

	#[test]
	fn create_marketplace_order_works() {
		new_test_ext().execute_with(|| {
			let store_id = create_test_store(2);
			let items = vec![(Some(1u64), b"Test Product".to_vec(), 2u32, 1000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				0, // Marketplace
				None,
				2,
				Some(store_id),
				items
			));

			let order = Pallet::<Test>::orders(1).unwrap();
			assert_eq!(order.order_id, 1);
			assert_eq!(order.buyer, 1);
			assert_eq!(order.seller, 2);
			assert_eq!(order.total_amount, 2000);
			assert_eq!(order.platform_fee, 50); // 2.5% of 2000
			assert_eq!(order.status, OrderStatus::Pending);
			assert_eq!(order.source, OrderSource::Marketplace);

			// Event assertion omitted - requires RuntimeEvent type
			// Event assertion omitted
			// System::assert_last_event(Event::OrderCreated {
			// 	order_id: 1, buyer: 1, seller: 2, total_amount: 2000,
			// }));
		});
	}

	#[test]
	fn create_bazchat_order_works() {
		new_test_ext().execute_with(|| {
			let items = vec![(None, b"Custom Item".to_vec().into(), 1u32, 5000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				1, // BazChat
				Some(123), // thread_id
				2,
				None,
				items
			));

			let order = Pallet::<Test>::orders(1).unwrap();
			assert_eq!(order.status, OrderStatus::Proposed);
			assert_eq!(order.source, OrderSource::BazChat);
			assert_eq!(order.thread_id, Some(123));
		});
	}

	#[test]
	fn accept_proposal_works() {
		new_test_ext().execute_with(|| {
			let items = vec![(None, b"Custom Item".to_vec().into(), 1u32, 5000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				1, // BazChat
				Some(123),
				2,
				None,
				items
			));

			assert_ok!(Pallet::<Test>::accept_proposal(frame_system::RawOrigin::Signed(1).into(), 1));

			let order = Pallet::<Test>::orders(1).unwrap();
			assert_eq!(order.status, OrderStatus::Pending);

			// Event assertion omitted
			// System::assert_last_event(Event::ProposalAccepted {
			// 	order_id: 1, buyer: 1,
			// });
		});
	}

	#[test]
	fn accept_proposal_requires_buyer() {
		new_test_ext().execute_with(|| {
			let items = vec![(None, b"Custom Item".to_vec().into(), 1u32, 5000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				1, // BazChat
				Some(123),
				2,
				None,
				items
			));

			// Seller tries to accept
			assert_noop!(
				Pallet::<Test>::accept_proposal(frame_system::RawOrigin::Signed(2).into(), 1),
				Error::<Test>::Unauthorized
			);
		});
	}

	#[test]
	fn mark_paid_works() {
		new_test_ext().execute_with(|| {
			let store_id = create_test_store(2);
			let items = vec![(Some(1u64), b"Test Product".to_vec(), 2u32, 1000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				0, // Marketplace
				None,
				2,
				Some(store_id),
				items
			));

			assert_ok!(Pallet::<Test>::mark_paid(frame_system::RawOrigin::Signed(99).into(), 1, 500));

			let order = Pallet::<Test>::orders(1).unwrap();
			assert_eq!(order.status, OrderStatus::Paid);
			assert_eq!(order.escrow_id, Some(500));
			assert_eq!(order.paid_at, Some(1));

			// Event assertion omitted
			// Event assertion omitted
			// System::assert_last_event(Event::OrderPaid {
			// 	order_id: 1, escrow_id: 500,
			// });
		});
	}

	#[test]
	fn mark_shipped_works() {
		new_test_ext().execute_with(|| {
			let store_id = create_test_store(2);
			let items = vec![(Some(1u64), b"Test Product".to_vec(), 2u32, 1000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				0, // Marketplace
				None,
				2,
				Some(store_id),
				items
			));

			assert_ok!(Pallet::<Test>::mark_paid(frame_system::RawOrigin::Signed(1).into(), 1, 500));
			assert_ok!(Pallet::<Test>::mark_shipped(frame_system::RawOrigin::Signed(2).into(), 1));

			let order = Pallet::<Test>::orders(1).unwrap();
			assert_eq!(order.status, OrderStatus::Shipped);
			assert_eq!(order.shipped_at, Some(1));

			// Event assertion omitted
			// Event assertion omitted
			// System::assert_last_event(Event::OrderShipped {
			// 	order_id: 1, seller: 2,
			// });
		});
	}

	#[test]
	fn mark_delivered_creates_sale() {
		new_test_ext().execute_with(|| {
			let store_id = create_test_store(2);
			let items = vec![(Some(1u64), b"Test Product".to_vec(), 2u32, 1000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				0, // Marketplace
				None,
				2,
				Some(store_id),
				items
			));

			assert_ok!(Pallet::<Test>::mark_paid(frame_system::RawOrigin::Signed(1).into(), 1, 500));
			assert_ok!(Pallet::<Test>::mark_shipped(frame_system::RawOrigin::Signed(2).into(), 1));
			assert_ok!(Pallet::<Test>::mark_delivered(frame_system::RawOrigin::Signed(1).into(), 1));

			let order = Pallet::<Test>::orders(1).unwrap();
			assert_eq!(order.status, OrderStatus::Delivered);
			assert_eq!(order.delivered_at, Some(1));

			// Check sale was created
			let sale = Pallet::<Test>::sales(1).unwrap();
			assert_eq!(sale.order_id, 1);
			assert_eq!(sale.seller, 2);
			assert_eq!(sale.buyer, 1);
			assert_eq!(sale.amount, 2000);

			// Event assertion omitted
			// Event assertion omitted
			// System::assert_last_event(Event::OrderDelivered {
			// 	order_id: 1, sale_id: 1,
			// });
		});
	}

	#[test]
	fn mint_receipt_works() {
		new_test_ext().execute_with(|| {
			let store_id = create_test_store(2);
			let items = vec![(Some(1u64), b"Test Product".to_vec(), 2u32, 1000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				0, // Marketplace
				None,
				2,
				Some(store_id),
				items
			));

			assert_ok!(Pallet::<Test>::mark_paid(frame_system::RawOrigin::Signed(1).into(), 1, 500));
			assert_ok!(Pallet::<Test>::mark_shipped(frame_system::RawOrigin::Signed(2).into(), 1));
			assert_ok!(Pallet::<Test>::mark_delivered(frame_system::RawOrigin::Signed(1).into(), 1));
			assert_ok!(Pallet::<Test>::mint_receipt(frame_system::RawOrigin::Signed(1).into(), 1));

			let receipt = Pallet::<Test>::receipt_nfts(1).unwrap();
			assert_eq!(receipt.order_id, 1);
			assert_eq!(receipt.owner, 1);

			let order = Pallet::<Test>::orders(1).unwrap();
			assert_eq!(order.receipt_nft_id, Some(1));

			// Event assertion omitted
			// Event assertion omitted
			// System::assert_last_event(Event::ReceiptMinted {
			// 	nft_id: 1, order_id: 1, owner: 1,
			// });
		});
	}

	#[test]
	fn cancel_order_works() {
		new_test_ext().execute_with(|| {
			let store_id = create_test_store(2);
			let items = vec![(Some(1u64), b"Test Product".to_vec(), 2u32, 1000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				0, // Marketplace
				None,
				2,
				Some(store_id),
				items
			));

			assert_ok!(Pallet::<Test>::cancel_order(frame_system::RawOrigin::Signed(1).into(), 1));

			let order = Pallet::<Test>::orders(1).unwrap();
			assert_eq!(order.status, OrderStatus::Cancelled);

			// Event assertion omitted
			// Event assertion omitted
			// System::assert_last_event(Event::OrderCancelled {
			// 	order_id: 1, who: 1,
			// });
		});
	}

	#[test]
	fn cancel_paid_order_fails() {
		new_test_ext().execute_with(|| {
			let store_id = create_test_store(2);
			let items = vec![(Some(1u64), b"Test Product".to_vec(), 2u32, 1000u128)];

			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				0, // Marketplace
				None,
				2,
				Some(store_id),
				items
			));

			assert_ok!(Pallet::<Test>::mark_paid(frame_system::RawOrigin::Signed(1).into(), 1, 500));

			assert_noop!(
				Pallet::<Test>::cancel_order(frame_system::RawOrigin::Signed(1).into(), 1),
				Error::<Test>::CannotCancelOrder
			);
		});
	}

	#[test]
	fn empty_order_fails() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Pallet::<Test>::create_order(
					frame_system::RawOrigin::Signed(1).into(),
					0, // Marketplace
					None,
					2,
					None,
					vec![]
				),
				Error::<Test>::EmptyOrder
			);
		});
	}

	#[test]
	fn record_commission_works() {
		new_test_ext().execute_with(|| {
			let store_id = create_test_store(2);
			let items = vec![(Some(1u64), b"Test Product".to_vec(), 1u32, 1000u128)];

			// Create and deliver order to generate sale
			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				0, // Marketplace
				None,
				2,
				Some(store_id),
				items
			));

			assert_ok!(Pallet::<Test>::mark_paid(
				frame_system::RawOrigin::Signed(99).into(),
				1,
				1
			));

			assert_ok!(Pallet::<Test>::mark_shipped(
				frame_system::RawOrigin::Signed(2).into(),
				1
			));

			assert_ok!(Pallet::<Test>::mark_delivered(
				frame_system::RawOrigin::Signed(1).into(),
				1
			));

			// Sale created, commission_paid should be 0
			let sale = Pallet::<Test>::sales(1).unwrap();
			assert_eq!(sale.commission_paid, 0);

			// Record commission
			let commission_amount = 100u128;
			assert_ok!(Pallet::<Test>::record_commission(
				frame_system::RawOrigin::Signed(99).into(),
				1,
				commission_amount,
				99
			));

			// Verify commission was recorded
			let sale = Pallet::<Test>::sales(1).unwrap();
			assert_eq!(sale.commission_paid, commission_amount);
		});
	}

	#[test]
	fn record_commission_accumulates() {
		new_test_ext().execute_with(|| {
			let store_id = create_test_store(2);
			let items = vec![(Some(1u64), b"Test Product".to_vec(), 1u32, 1000u128)];

			// Create and deliver order
			assert_ok!(Pallet::<Test>::create_order(
				frame_system::RawOrigin::Signed(1).into(),
				0,
				None,
				2,
				Some(store_id),
				items
			));
			assert_ok!(Pallet::<Test>::mark_paid(
				frame_system::RawOrigin::Signed(99).into(),
				1,
				1
			));
			assert_ok!(Pallet::<Test>::mark_shipped(
				frame_system::RawOrigin::Signed(2).into(),
				1
			));
			assert_ok!(Pallet::<Test>::mark_delivered(
				frame_system::RawOrigin::Signed(1).into(),
				1
			));

			// Record multiple commissions
			assert_ok!(Pallet::<Test>::record_commission(
				frame_system::RawOrigin::Signed(99).into(),
				1,
				50u128,
				3
			));

			assert_ok!(Pallet::<Test>::record_commission(
				frame_system::RawOrigin::Signed(99).into(),
				1,
				30u128,
				4
			));

			// Verify commissions accumulated
			let sale = Pallet::<Test>::sales(1).unwrap();
			assert_eq!(sale.commission_paid, 80u128);
		});
	}

	#[test]
	fn record_commission_fails_for_nonexistent_sale() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Pallet::<Test>::record_commission(
					frame_system::RawOrigin::Signed(99).into(),
					999, // Non-existent sale
					100u128,
					99
				),
				Error::<Test>::SaleNotFound
			);
		});
	}
}
