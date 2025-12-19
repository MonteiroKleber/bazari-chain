//! # Bazari Recurring Payments Pallet
//!
//! A pallet for managing recurring payment contracts on-chain.
//! Part of the Bazari Pay system.
//!
//! ## Overview
//!
//! This pallet allows:
//! - Creating recurring payment contracts between payer and receiver
//! - Executing scheduled payments
//! - Updating contract status (pause, resume, close)
//! - Querying contract and execution history

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{Saturating, Zero};

    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency mechanism.
        type Currency: ReservableCurrency<Self::AccountId>;

        /// Maximum contracts per account (for bounded storage).
        #[pallet::constant]
        type MaxContractsPerAccount: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Payment period enumeration.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum PaymentPeriod {
        Weekly,
        Biweekly,
        Monthly,
    }

    impl Default for PaymentPeriod {
        fn default() -> Self {
            PaymentPeriod::Monthly
        }
    }

    // Manual implementation for DecodeWithMemTracking
    impl codec::DecodeWithMemTracking for PaymentPeriod {}

    /// Contract status enumeration.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum ContractStatus {
        Active,
        Paused,
        Closed,
    }

    impl Default for ContractStatus {
        fn default() -> Self {
            ContractStatus::Active
        }
    }

    // Manual implementation for DecodeWithMemTracking
    impl codec::DecodeWithMemTracking for ContractStatus {}

    /// A recurring payment contract.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct RecurringContract<T: Config> {
        /// Unique contract identifier (blake2 hash of off-chain ID).
        pub id: [u8; 32],
        /// Account that pays.
        pub payer: T::AccountId,
        /// Account that receives.
        pub receiver: T::AccountId,
        /// Base payment value.
        pub base_value: BalanceOf<T>,
        /// Payment period (weekly, biweekly, monthly).
        pub period: PaymentPeriod,
        /// Day of period for payment (1-31 for monthly, 1-7 for weekly).
        pub payment_day: u8,
        /// Current contract status.
        pub status: ContractStatus,
        /// Block number when contract was created.
        pub created_at: BlockNumberFor<T>,
        /// Block number of next scheduled payment.
        pub next_payment: BlockNumberFor<T>,
        /// Total number of executions.
        pub execution_count: u32,
        /// Total amount paid over all executions.
        pub total_paid: BalanceOf<T>,
    }

    /// A payment execution record.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct PaymentExecution<Balance, BlockNumber> {
        /// Unique execution identifier.
        pub id: [u8; 32],
        /// Period reference (e.g., "2025-02" encoded as bytes).
        pub period_ref: [u8; 7],
        /// Value paid in this execution.
        pub value_paid: Balance,
        /// Block number when executed.
        pub executed_at: BlockNumber,
    }

    /// Storage: Contracts by ID.
    #[pallet::storage]
    #[pallet::getter(fn contracts)]
    pub type Contracts<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], RecurringContract<T>>;

    /// Storage: Executions by contract ID and execution ID.
    #[pallet::storage]
    #[pallet::getter(fn executions)]
    pub type Executions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        [u8; 32], // contract_id
        Blake2_128Concat,
        [u8; 32], // execution_id
        PaymentExecution<BalanceOf<T>, BlockNumberFor<T>>,
    >;

    /// Storage: Contract IDs by payer.
    #[pallet::storage]
    #[pallet::getter(fn contracts_by_payer)]
    pub type ContractsByPayer<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, [u8; 32], ()>;

    /// Storage: Contract IDs by receiver.
    #[pallet::storage]
    #[pallet::getter(fn contracts_by_receiver)]
    pub type ContractsByReceiver<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, [u8; 32], ()>;

    /// Storage: Execution count per contract.
    #[pallet::storage]
    #[pallet::getter(fn execution_count)]
    pub type ExecutionCount<T: Config> = StorageMap<_, Blake2_128Concat, [u8; 32], u32, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new recurring payment contract was created.
        ContractCreated {
            id: [u8; 32],
            payer: T::AccountId,
            receiver: T::AccountId,
            base_value: BalanceOf<T>,
            period: PaymentPeriod,
        },
        /// Contract status was updated.
        ContractStatusUpdated {
            id: [u8; 32],
            old_status: ContractStatus,
            new_status: ContractStatus,
        },
        /// A payment was executed.
        PaymentExecuted {
            contract_id: [u8; 32],
            execution_id: [u8; 32],
            value: BalanceOf<T>,
            period_ref: [u8; 7],
        },
        /// Payment execution failed.
        PaymentFailed {
            contract_id: [u8; 32],
            execution_id: [u8; 32],
            reason: PaymentFailureReason,
        },
    }

    /// Reasons for payment failure.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum PaymentFailureReason {
        InsufficientBalance,
        TransferFailed,
        ContractNotActive,
    }

    impl Default for PaymentFailureReason {
        fn default() -> Self {
            PaymentFailureReason::ContractNotActive
        }
    }

    // Manual implementation for DecodeWithMemTracking
    impl codec::DecodeWithMemTracking for PaymentFailureReason {}

    #[pallet::error]
    pub enum Error<T> {
        /// Contract with this ID already exists.
        ContractAlreadyExists,
        /// Contract not found.
        ContractNotFound,
        /// Caller is not authorized for this action.
        NotAuthorized,
        /// Invalid status transition.
        InvalidStatusTransition,
        /// Insufficient balance for payment.
        InsufficientBalance,
        /// Transfer failed.
        TransferFailed,
        /// Contract is not active.
        ContractNotActive,
        /// Execution already exists.
        ExecutionAlreadyExists,
        /// Invalid payment day.
        InvalidPaymentDay,
        /// Maximum contracts per account reached.
        MaxContractsReached,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create a new recurring payment contract.
        ///
        /// The caller becomes the payer.
        ///
        /// # Arguments
        /// * `id` - Unique identifier (blake2 hash of off-chain contract ID)
        /// * `receiver` - Account to receive payments
        /// * `base_value` - Base payment amount
        /// * `period` - Payment period (Weekly, Biweekly, Monthly)
        /// * `payment_day` - Day of period for payment
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn create_contract(
            origin: OriginFor<T>,
            id: [u8; 32],
            receiver: T::AccountId,
            base_value: BalanceOf<T>,
            period: PaymentPeriod,
            payment_day: u8,
        ) -> DispatchResult {
            let payer = ensure_signed(origin)?;

            // Validate payment day
            let max_day = match period {
                PaymentPeriod::Weekly => 7,
                PaymentPeriod::Biweekly => 14,
                PaymentPeriod::Monthly => 31,
            };
            ensure!(payment_day >= 1 && payment_day <= max_day, Error::<T>::InvalidPaymentDay);

            // Check contract doesn't exist
            ensure!(!Contracts::<T>::contains_key(&id), Error::<T>::ContractAlreadyExists);

            let current_block = frame_system::Pallet::<T>::block_number();
            let next_payment = Self::calculate_next_payment(current_block, &period, payment_day);

            let contract = RecurringContract {
                id,
                payer: payer.clone(),
                receiver: receiver.clone(),
                base_value,
                period: period.clone(),
                payment_day,
                status: ContractStatus::Active,
                created_at: current_block,
                next_payment,
                execution_count: 0,
                total_paid: Zero::zero(),
            };

            // Store contract and indexes
            Contracts::<T>::insert(&id, contract);
            ContractsByPayer::<T>::insert(&payer, &id, ());
            ContractsByReceiver::<T>::insert(&receiver, &id, ());

            Self::deposit_event(Event::ContractCreated {
                id,
                payer,
                receiver,
                base_value,
                period,
            });

            Ok(())
        }

        /// Update the status of a contract.
        ///
        /// Only the payer or receiver can update status.
        /// Valid transitions:
        /// - Active -> Paused
        /// - Active -> Closed
        /// - Paused -> Active
        /// - Paused -> Closed
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn update_status(
            origin: OriginFor<T>,
            id: [u8; 32],
            new_status: ContractStatus,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Contracts::<T>::try_mutate(&id, |maybe_contract| {
                let contract = maybe_contract.as_mut().ok_or(Error::<T>::ContractNotFound)?;

                // Only payer or receiver can update
                ensure!(
                    who == contract.payer || who == contract.receiver,
                    Error::<T>::NotAuthorized
                );

                let old_status = contract.status.clone();

                // Validate transition
                ensure!(
                    Self::is_valid_transition(&old_status, &new_status),
                    Error::<T>::InvalidStatusTransition
                );

                contract.status = new_status.clone();

                Self::deposit_event(Event::ContractStatusUpdated {
                    id,
                    old_status,
                    new_status,
                });

                Ok(())
            })
        }

        /// Execute a payment for a contract.
        ///
        /// This transfers funds from payer to receiver and records the execution.
        /// Can be called by anyone (typically an off-chain worker or scheduler).
        ///
        /// # Arguments
        /// * `contract_id` - The contract to execute payment for
        /// * `execution_id` - Unique identifier for this execution
        /// * `period_ref` - Period reference (e.g., "2025-02")
        /// * `value` - Amount to transfer (may include adjustments)
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(15_000, 0))]
        pub fn execute_payment(
            origin: OriginFor<T>,
            contract_id: [u8; 32],
            execution_id: [u8; 32],
            period_ref: [u8; 7],
            value: BalanceOf<T>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // Get contract
            let mut contract =
                Contracts::<T>::get(&contract_id).ok_or(Error::<T>::ContractNotFound)?;

            // Must be active
            ensure!(contract.status == ContractStatus::Active, Error::<T>::ContractNotActive);

            // Check execution doesn't exist
            ensure!(
                !Executions::<T>::contains_key(&contract_id, &execution_id),
                Error::<T>::ExecutionAlreadyExists
            );

            // Check balance
            ensure!(
                T::Currency::free_balance(&contract.payer) >= value,
                Error::<T>::InsufficientBalance
            );

            // Execute transfer
            T::Currency::transfer(
                &contract.payer,
                &contract.receiver,
                value,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::TransferFailed)?;

            // Record execution
            let current_block = frame_system::Pallet::<T>::block_number();
            let execution = PaymentExecution {
                id: execution_id,
                period_ref,
                value_paid: value,
                executed_at: current_block,
            };

            Executions::<T>::insert(&contract_id, &execution_id, execution);
            ExecutionCount::<T>::mutate(&contract_id, |count| *count = count.saturating_add(1));

            // Update contract
            contract.execution_count = contract.execution_count.saturating_add(1);
            contract.total_paid = contract.total_paid.saturating_add(value);
            contract.next_payment =
                Self::calculate_next_payment(current_block, &contract.period, contract.payment_day);

            Contracts::<T>::insert(&contract_id, contract);

            Self::deposit_event(Event::PaymentExecuted {
                contract_id,
                execution_id,
                value,
                period_ref,
            });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Check if a status transition is valid.
        fn is_valid_transition(from: &ContractStatus, to: &ContractStatus) -> bool {
            matches!(
                (from, to),
                (ContractStatus::Active, ContractStatus::Paused)
                    | (ContractStatus::Active, ContractStatus::Closed)
                    | (ContractStatus::Paused, ContractStatus::Active)
                    | (ContractStatus::Paused, ContractStatus::Closed)
            )
        }

        /// Calculate the next payment block number.
        /// Simplified: adds ~1 month worth of blocks (assuming 6s/block).
        fn calculate_next_payment(
            current_block: BlockNumberFor<T>,
            period: &PaymentPeriod,
            _payment_day: u8,
        ) -> BlockNumberFor<T> {
            let blocks_per_period: u32 = match period {
                PaymentPeriod::Weekly => 100_800,    // 7 days * 24h * 60m * 10 blocks/min
                PaymentPeriod::Biweekly => 201_600,  // 14 days
                PaymentPeriod::Monthly => 432_000,   // ~30 days
            };

            current_block.saturating_add(blocks_per_period.into())
        }
    }
}
