//! # Bazari Work Agreements Pallet
//!
//! Pallet para registro on-chain de acordos de trabalho.
//! Armazena apenas metadados mínimos como prova imutável de vínculo.
//!
//! ## Princípio
//! - On-chain: ID hash, wallets, tipo de pagamento, status, timestamps
//! - Off-chain: título, descrição, valores, mensagens, detalhes

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use codec::{Decode, Encode, MaxEncodedLen};
    use scale_info::TypeInfo;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Tipo de pagamento do acordo
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum PaymentType {
        /// Pagamento externo (fora do Bazari)
        External,
        /// Pagamento via Bazari Pay
        BazariPay,
        /// A definir
        Undefined,
    }

    impl Default for PaymentType {
        fn default() -> Self {
            PaymentType::Undefined
        }
    }

    // Manual implementation for DecodeWithMemTracking
    impl codec::DecodeWithMemTracking for PaymentType {}

    /// Status do acordo
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum AgreementStatus {
        /// Acordo ativo
        Active,
        /// Acordo pausado temporariamente
        Paused,
        /// Acordo encerrado
        Closed,
    }

    impl Default for AgreementStatus {
        fn default() -> Self {
            AgreementStatus::Active
        }
    }

    // Manual implementation for DecodeWithMemTracking
    impl codec::DecodeWithMemTracking for AgreementStatus {}

    /// Registro de acordo on-chain
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct WorkAgreementOnChain<T: Config> {
        /// Hash do ID off-chain (32 bytes, Blake2-256)
        pub id_hash: [u8; 32],
        /// Wallet da empresa/contratante
        pub company: T::AccountId,
        /// Wallet do trabalhador
        pub worker: T::AccountId,
        /// Tipo de pagamento
        pub payment_type: PaymentType,
        /// Status atual
        pub status: AgreementStatus,
        /// Block number de criação
        pub created_at: BlockNumberFor<T>,
        /// Block number de encerramento (se fechado)
        pub closed_at: Option<BlockNumberFor<T>>,
    }

    /// Storage: Acordos por id_hash
    #[pallet::storage]
    #[pallet::getter(fn agreements)]
    pub type Agreements<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        [u8; 32],  // id_hash
        WorkAgreementOnChain<T>,
    >;

    /// Storage: Índice de acordos por empresa
    #[pallet::storage]
    #[pallet::getter(fn agreements_by_company)]
    pub type AgreementsByCompany<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,  // company
        Blake2_128Concat,
        [u8; 32],      // id_hash
        (),
    >;

    /// Storage: Índice de acordos por trabalhador
    #[pallet::storage]
    #[pallet::getter(fn agreements_by_worker)]
    pub type AgreementsByWorker<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,  // worker
        Blake2_128Concat,
        [u8; 32],      // id_hash
        (),
    >;

    /// Storage: Contador de acordos
    #[pallet::storage]
    #[pallet::getter(fn agreement_count)]
    pub type AgreementCount<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Acordo registrado on-chain
        AgreementCreated {
            id_hash: [u8; 32],
            company: T::AccountId,
            worker: T::AccountId,
            payment_type: PaymentType,
        },
        /// Status do acordo atualizado
        AgreementStatusUpdated {
            id_hash: [u8; 32],
            old_status: AgreementStatus,
            new_status: AgreementStatus,
            updated_by: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Acordo já existe com este ID
        AgreementAlreadyExists,
        /// Acordo não encontrado
        AgreementNotFound,
        /// Não autorizado a modificar este acordo
        NotAuthorized,
        /// Transição de status inválida
        InvalidStatusTransition,
        /// Acordo já está fechado
        AgreementAlreadyClosed,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Registrar novo acordo on-chain
        ///
        /// - `id_hash`: Hash Blake2-256 do ID off-chain
        /// - `worker`: Wallet do trabalhador
        /// - `payment_type`: Tipo de pagamento acordado
        ///
        /// O caller (origin) é registrado como a empresa/contratante.
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn create_agreement(
            origin: OriginFor<T>,
            id_hash: [u8; 32],
            worker: T::AccountId,
            payment_type: PaymentType,
        ) -> DispatchResult {
            let company = ensure_signed(origin)?;

            // Verificar se acordo já existe
            ensure!(
                !Agreements::<T>::contains_key(&id_hash),
                Error::<T>::AgreementAlreadyExists
            );

            let current_block = frame_system::Pallet::<T>::block_number();

            let agreement = WorkAgreementOnChain {
                id_hash,
                company: company.clone(),
                worker: worker.clone(),
                payment_type: payment_type.clone(),
                status: AgreementStatus::Active,
                created_at: current_block,
                closed_at: None,
            };

            // Inserir acordo
            Agreements::<T>::insert(&id_hash, agreement);

            // Criar índices
            AgreementsByCompany::<T>::insert(&company, &id_hash, ());
            AgreementsByWorker::<T>::insert(&worker, &id_hash, ());

            // Incrementar contador
            AgreementCount::<T>::mutate(|count| *count = count.saturating_add(1));

            Self::deposit_event(Event::AgreementCreated {
                id_hash,
                company,
                worker,
                payment_type,
            });

            Ok(())
        }

        /// Atualizar status do acordo
        ///
        /// - `id_hash`: Hash do acordo
        /// - `new_status`: Novo status
        ///
        /// Apenas empresa ou trabalhador podem atualizar.
        /// Transições válidas:
        /// - Active -> Paused
        /// - Active -> Closed
        /// - Paused -> Active
        /// - Paused -> Closed
        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn update_status(
            origin: OriginFor<T>,
            id_hash: [u8; 32],
            new_status: AgreementStatus,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Agreements::<T>::try_mutate(&id_hash, |maybe_agreement| {
                let agreement = maybe_agreement.as_mut()
                    .ok_or(Error::<T>::AgreementNotFound)?;

                // Verificar autorização (empresa ou trabalhador)
                ensure!(
                    who == agreement.company || who == agreement.worker,
                    Error::<T>::NotAuthorized
                );

                // Verificar se já está fechado
                ensure!(
                    agreement.status != AgreementStatus::Closed,
                    Error::<T>::AgreementAlreadyClosed
                );

                // Validar transição de status
                let old_status = agreement.status.clone();
                ensure!(
                    Self::is_valid_transition(&old_status, &new_status),
                    Error::<T>::InvalidStatusTransition
                );

                // Atualizar status
                agreement.status = new_status.clone();

                // Se fechando, registrar block number
                if new_status == AgreementStatus::Closed {
                    agreement.closed_at = Some(frame_system::Pallet::<T>::block_number());
                }

                Self::deposit_event(Event::AgreementStatusUpdated {
                    id_hash,
                    old_status,
                    new_status,
                    updated_by: who,
                });

                Ok(())
            })
        }
    }

    impl<T: Config> Pallet<T> {
        /// Verifica se a transição de status é válida
        fn is_valid_transition(from: &AgreementStatus, to: &AgreementStatus) -> bool {
            match (from, to) {
                // De Active
                (AgreementStatus::Active, AgreementStatus::Paused) => true,
                (AgreementStatus::Active, AgreementStatus::Closed) => true,
                // De Paused
                (AgreementStatus::Paused, AgreementStatus::Active) => true,
                (AgreementStatus::Paused, AgreementStatus::Closed) => true,
                // Qualquer outra transição é inválida
                _ => false,
            }
        }

        /// Retorna a quantidade de acordos de uma empresa
        pub fn company_agreement_count(company: &T::AccountId) -> u32 {
            AgreementsByCompany::<T>::iter_prefix(company).count() as u32
        }

        /// Retorna a quantidade de acordos de um trabalhador
        pub fn worker_agreement_count(worker: &T::AccountId) -> u32 {
            AgreementsByWorker::<T>::iter_prefix(worker).count() as u32
        }
    }
}
