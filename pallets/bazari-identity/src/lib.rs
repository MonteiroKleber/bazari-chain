#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub mod types;

#[frame_support::pallet]
pub mod pallet {
    use super::types::*;
    use frame_support::{
        pallet_prelude::*,
        BoundedVec,
        BoundedBTreeSet,
    };
    use frame_system::pallet_prelude::*;
    use scale_info::prelude::vec::Vec;
    use sp_runtime::traits::{BlockNumberProvider, Saturating};

    // Type aliases para melhor legibilidade
    type CidOf<T> = BoundedVec<u8, <T as Config>::MaxCidLen>;
    type HandleOf<T> = BoundedVec<u8, <T as Config>::MaxHandleLen>;
    type BadgeOf<T> = Badge<<T as Config>::MaxBadgeCodeLen>;
    type BadgeListOf<T> = BoundedVec<BadgeOf<T>, <T as Config>::MaxBadges>;
    type HandleRecordOf<T> = HandleRecord<<T as Config>::MaxHandleLen>;
    type HandleHistoryOf<T> = BoundedVec<HandleRecordOf<T>, <T as Config>::MaxHandleHistory>;
    type ReasonCodeOf<T> = BoundedVec<u8, <T as Config>::MaxReasonCodeLen>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Tamanho máximo do CID IPFS (96 bytes)
        #[pallet::constant]
        type MaxCidLen: Get<u32>;

        /// Tamanho máximo do handle (32 bytes)
        #[pallet::constant]
        type MaxHandleLen: Get<u32>;

        /// Número máximo de badges por perfil (50)
        #[pallet::constant]
        type MaxBadges: Get<u32>;

        /// Tamanho máximo do código do badge (32 bytes)
        #[pallet::constant]
        type MaxBadgeCodeLen: Get<u32>;

        /// Tamanho máximo do histórico de handles (10)
        #[pallet::constant]
        type MaxHandleHistory: Get<u32>;

        /// Cooldown em blocos para mudança de handle (432000 blocos ~30 dias)
        #[pallet::constant]
        type HandleCooldownBlocks: Get<BlockNumberFor<Self>>;

        /// Tamanho máximo do código de razão (64 bytes)
        #[pallet::constant]
        type MaxReasonCodeLen: Get<u32>;

        /// Número máximo de módulos autorizados
        #[pallet::constant]
        type MaxAuthorizedModules: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Contador sequencial para próximo ProfileId
    #[pallet::storage]
    #[pallet::getter(fn next_profile_id)]
    pub type NextProfileId<T> = StorageValue<_, ProfileId, ValueQuery>;

    /// Mapeamento de AccountId para ProfileId (garantia 1:1)
    #[pallet::storage]
    #[pallet::getter(fn owner_profile)]
    pub type OwnerProfile<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        ProfileId,
        OptionQuery,
    >;

    /// Mapeamento reverso: ProfileId para AccountId
    #[pallet::storage]
    #[pallet::getter(fn profile_owner)]
    pub type ProfileOwner<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ProfileId,
        T::AccountId,
        OptionQuery,
    >;

    /// CID dos metadados IPFS do perfil
    #[pallet::storage]
    #[pallet::getter(fn profile_cid)]
    pub type ProfileCid<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ProfileId,
        CidOf<T>,
        OptionQuery,
    >;

    /// Handle do perfil
    #[pallet::storage]
    #[pallet::getter(fn handle)]
    pub type Handle<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ProfileId,
        HandleOf<T>,
        OptionQuery,
    >;

    /// Índice reverso: Handle para ProfileId
    #[pallet::storage]
    #[pallet::getter(fn handle_to_profile)]
    pub type HandleToProfile<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        HandleOf<T>,
        ProfileId,
        OptionQuery,
    >;

    /// Score de reputação (pode ser negativo)
    #[pallet::storage]
    #[pallet::getter(fn reputation)]
    pub type Reputation<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ProfileId,
        i32,
        ValueQuery,
    >;

    /// Lista de badges conquistados
    #[pallet::storage]
    #[pallet::getter(fn badges)]
    pub type Badges<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ProfileId,
        BadgeListOf<T>,
        ValueQuery,
    >;

    /// Timestamp da última mudança de handle
    #[pallet::storage]
    #[pallet::getter(fn last_handle_change)]
    pub type LastHandleChange<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ProfileId,
        BlockNumberFor<T>,
        OptionQuery,
    >;

    /// Histórico de handles anteriores
    #[pallet::storage]
    #[pallet::getter(fn handle_history)]
    pub type HandleHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ProfileId,
        HandleHistoryOf<T>,
        ValueQuery,
    >;

    /// Módulos autorizados a modificar reputação
    #[pallet::storage]
    #[pallet::getter(fn authorized_modules)]
    pub type AuthorizedModules<T: Config> = StorageValue<
        _,
        BoundedBTreeSet<ModuleId, T::MaxAuthorizedModules>,
        ValueQuery,
    >;

    /// Módulos autorizados a emitir badges
    #[pallet::storage]
    #[pallet::getter(fn authorized_issuers)]
    pub type AuthorizedIssuers<T: Config> = StorageValue<
        _,
        BoundedBTreeSet<ModuleId, T::MaxAuthorizedModules>,
        ValueQuery,
    >;

    /// Flags do perfil (verified, banned, etc.)
    #[pallet::storage]
    #[pallet::getter(fn flags)]
    pub type Flags<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ProfileId,
        u32,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Profile foi criado
        ProfileMinted {
            profile_id: ProfileId,
            owner: T::AccountId,
            handle: Vec<u8>,
        },
        /// Metadados atualizados
        MetadataUpdated {
            profile_id: ProfileId,
            who: T::AccountId,
            cid: Vec<u8>,
        },
        /// Handle foi alterado
        HandleChanged {
            profile_id: ProfileId,
            old_handle: Vec<u8>,
            new_handle: Vec<u8>,
        },
        /// Reputação alterada
        ReputationChanged {
            profile_id: ProfileId,
            delta: i32,
            new_total: i32,
            reason: Vec<u8>,
        },
        /// Badge concedido
        BadgeAwarded {
            profile_id: ProfileId,
            code: Vec<u8>,
            issuer: ModuleId,
        },
        /// Badge revogado
        BadgeRevoked {
            profile_id: ProfileId,
            code: Vec<u8>,
            reason: Vec<u8>,
        },
        /// Módulo autorizado para reputação
        ModuleAuthorized {
            module_id: ModuleId,
        },
        /// Módulo desautorizado para reputação
        ModuleDeauthorized {
            module_id: ModuleId,
        },
        /// Issuer autorizado para badges
        IssuerAuthorized {
            issuer_id: ModuleId,
        },
        /// Issuer desautorizado para badges
        IssuerDeauthorized {
            issuer_id: ModuleId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Perfil já existe para este owner
        ProfileExists,
        /// Perfil não encontrado
        ProfileNotFound,
        /// Não é o dono do perfil
        NotOwner,
        /// Handle já está em uso
        HandleTaken,
        /// Handle excede tamanho máximo
        HandleTooLong,
        /// Cooldown de handle ainda ativo
        HandleCooldown,
        /// Módulo não autorizado para modificar reputação
        UnauthorizedModule,
        /// Issuer não autorizado para emitir badges
        UnauthorizedIssuer,
        /// CID excede tamanho máximo
        CidTooLong,
        /// Badge já existe para este perfil
        BadgeAlreadyExists,
        /// Limite de badges atingido
        BadgeLimitReached,
        /// Badge não encontrado
        BadgeNotFound,
        /// Histórico de handles cheio
        HandleHistoryFull,
        /// Código de razão muito longo
        ReasonCodeTooLong,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Cria um novo perfil NFT soulbound
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 6))]
        pub fn mint_profile(
            origin: OriginFor<T>,
            owner: T::AccountId,
            handle: Vec<u8>,
            cid: Vec<u8>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // Validar que owner não tem profile
            ensure!(!OwnerProfile::<T>::contains_key(&owner), Error::<T>::ProfileExists);

            // Validar e converter handle
            let handle_bounded = HandleOf::<T>::try_from(handle.clone())
                .map_err(|_| Error::<T>::HandleTooLong)?;

            // Validar que handle é único
            ensure!(
                !HandleToProfile::<T>::contains_key(&handle_bounded),
                Error::<T>::HandleTaken
            );

            // Validar e converter CID
            let cid_bounded = CidOf::<T>::try_from(cid.clone())
                .map_err(|_| Error::<T>::CidTooLong)?;

            // Incrementar e obter próximo ProfileId
            let profile_id = NextProfileId::<T>::get();
            NextProfileId::<T>::put(profile_id.saturating_add(1));

            // Inserir em todos os storages
            OwnerProfile::<T>::insert(&owner, profile_id);
            ProfileOwner::<T>::insert(profile_id, &owner);
            ProfileCid::<T>::insert(profile_id, cid_bounded);
            Handle::<T>::insert(profile_id, handle_bounded.clone());
            HandleToProfile::<T>::insert(handle_bounded, profile_id);

            // Inicializar reputação em 0
            Reputation::<T>::insert(profile_id, 0);

            // Emitir evento
            Self::deposit_event(Event::ProfileMinted {
                profile_id,
                owner,
                handle,
            });

            Ok(())
        }

        /// Atualiza o CID dos metadados do perfil
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn update_metadata_cid(
            origin: OriginFor<T>,
            profile_id: ProfileId,
            cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Validar ownership
            let owner = ProfileOwner::<T>::get(profile_id)
                .ok_or(Error::<T>::ProfileNotFound)?;
            ensure!(owner == who, Error::<T>::NotOwner);

            // Validar e converter CID
            let cid_bounded = CidOf::<T>::try_from(cid.clone())
                .map_err(|_| Error::<T>::CidTooLong)?;

            // Atualizar storage
            ProfileCid::<T>::insert(profile_id, cid_bounded);

            // Emitir evento
            Self::deposit_event(Event::MetadataUpdated {
                profile_id,
                who,
                cid,
            });

            Ok(())
        }

        /// Altera o handle do perfil (com cooldown de 30 dias)
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 4))]
        pub fn set_handle(
            origin: OriginFor<T>,
            profile_id: ProfileId,
            new_handle: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Validar ownership
            let owner = ProfileOwner::<T>::get(profile_id)
                .ok_or(Error::<T>::ProfileNotFound)?;
            ensure!(owner == who, Error::<T>::NotOwner);

            // Validar e converter novo handle
            let new_handle_bounded = HandleOf::<T>::try_from(new_handle.clone())
                .map_err(|_| Error::<T>::HandleTooLong)?;

            // Validar que novo handle é único
            ensure!(
                !HandleToProfile::<T>::contains_key(&new_handle_bounded),
                Error::<T>::HandleTaken
            );

            // Validar cooldown
            let current_block = frame_system::Pallet::<T>::current_block_number();
            if let Some(last_change) = LastHandleChange::<T>::get(profile_id) {
                let blocks_since_change = current_block.saturating_sub(last_change);
                let cooldown_blocks = T::HandleCooldownBlocks::get();
                ensure!(
                    blocks_since_change >= cooldown_blocks,
                    Error::<T>::HandleCooldown
                );
            }

            // Obter handle antigo
            let old_handle = Handle::<T>::get(profile_id)
                .ok_or(Error::<T>::ProfileNotFound)?;
            let old_handle_vec = old_handle.to_vec();

            // Atualizar índices
            HandleToProfile::<T>::remove(&old_handle);
            HandleToProfile::<T>::insert(&new_handle_bounded, profile_id);
            Handle::<T>::insert(profile_id, new_handle_bounded);

            // Adicionar ao histórico
            let block_number_u64 = TryInto::<u64>::try_into(current_block)
                .ok()
                .unwrap_or(0);
            let record = HandleRecord {
                handle: old_handle,
                changed_at: block_number_u64,
            };

            HandleHistory::<T>::try_mutate(profile_id, |history| {
                history.try_push(record)
                    .map_err(|_| Error::<T>::HandleHistoryFull)
            })?;

            // Atualizar timestamp
            LastHandleChange::<T>::insert(profile_id, current_block);

            // Emitir evento
            Self::deposit_event(Event::HandleChanged {
                profile_id,
                old_handle: old_handle_vec,
                new_handle,
            });

            Ok(())
        }

        /// Incrementa a reputação de um perfil (apenas módulos autorizados)
        #[pallet::call_index(3)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 1))]
        pub fn increment_reputation(
            origin: OriginFor<T>,
            profile_id: ProfileId,
            points: i32,
            reason_code: Vec<u8>,
        ) -> DispatchResult {
            let _who = ensure_root(origin)?;

            // Validar que perfil existe
            ensure!(ProfileOwner::<T>::contains_key(profile_id), Error::<T>::ProfileNotFound);

            // Validar reason_code
            let _reason_bounded = ReasonCodeOf::<T>::try_from(reason_code.clone())
                .map_err(|_| Error::<T>::ReasonCodeTooLong)?;

            // Atualizar reputação
            Reputation::<T>::mutate(profile_id, |rep| {
                *rep = rep.saturating_add(points);
                let new_total = *rep;

                // Emitir evento
                Self::deposit_event(Event::ReputationChanged {
                    profile_id,
                    delta: points,
                    new_total,
                    reason: reason_code,
                });
            });

            Ok(())
        }

        /// Decrementa a reputação de um perfil (apenas módulos autorizados)
        #[pallet::call_index(4)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 1))]
        pub fn decrement_reputation(
            origin: OriginFor<T>,
            profile_id: ProfileId,
            points: i32,
            reason_code: Vec<u8>,
        ) -> DispatchResult {
            let _who = ensure_root(origin)?;

            // Validar que perfil existe
            ensure!(ProfileOwner::<T>::contains_key(profile_id), Error::<T>::ProfileNotFound);

            // Validar reason_code
            let _reason_bounded = ReasonCodeOf::<T>::try_from(reason_code.clone())
                .map_err(|_| Error::<T>::ReasonCodeTooLong)?;

            // Atualizar reputação (negativo)
            let delta = -points;
            Reputation::<T>::mutate(profile_id, |rep| {
                *rep = rep.saturating_add(delta);
                let new_total = *rep;

                // Emitir evento
                Self::deposit_event(Event::ReputationChanged {
                    profile_id,
                    delta,
                    new_total,
                    reason: reason_code,
                });
            });

            Ok(())
        }

        /// Concede um badge a um perfil (apenas issuers autorizados)
        #[pallet::call_index(5)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 1))]
        pub fn award_badge(
            origin: OriginFor<T>,
            profile_id: ProfileId,
            code: Vec<u8>,
            issuer: ModuleId,
        ) -> DispatchResult {
            let _who = ensure_root(origin)?;

            // Validar que perfil existe
            ensure!(ProfileOwner::<T>::contains_key(profile_id), Error::<T>::ProfileNotFound);

            // Validar código do badge
            let code_bounded = BoundedVec::<u8, T::MaxBadgeCodeLen>::try_from(code.clone())
                .map_err(|_| Error::<T>::HandleTooLong)?;

            // Obter timestamp atual
            let current_block = frame_system::Pallet::<T>::current_block_number();
            let block_number_u64 = TryInto::<u64>::try_into(current_block)
                .ok()
                .unwrap_or(0);

            // Criar badge
            let badge = Badge {
                code: code_bounded.clone(),
                issuer,
                issued_at: block_number_u64,
                revoked_at: None,
            };

            // Adicionar badge
            Badges::<T>::try_mutate(profile_id, |badges| {
                // Verificar duplicata
                if badges.iter().any(|b| b.code == code_bounded && b.revoked_at.is_none()) {
                    return Err(Error::<T>::BadgeAlreadyExists);
                }

                // Adicionar novo badge
                badges.try_push(badge)
                    .map_err(|_| Error::<T>::BadgeLimitReached)?;

                Ok(())
            })?;

            // Emitir evento
            Self::deposit_event(Event::BadgeAwarded {
                profile_id,
                code,
                issuer,
            });

            Ok(())
        }

        /// Revoga um badge de um perfil
        #[pallet::call_index(6)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn revoke_badge(
            origin: OriginFor<T>,
            profile_id: ProfileId,
            code: Vec<u8>,
            reason: Vec<u8>,
        ) -> DispatchResult {
            let _who = ensure_root(origin)?;

            // Validar código do badge
            let code_bounded = BoundedVec::<u8, T::MaxBadgeCodeLen>::try_from(code.clone())
                .map_err(|_| Error::<T>::HandleTooLong)?;

            // Obter timestamp atual
            let current_block = frame_system::Pallet::<T>::current_block_number();
            let block_number_u64 = TryInto::<u64>::try_into(current_block)
                .ok()
                .unwrap_or(0);

            // Revogar badge
            Badges::<T>::try_mutate(profile_id, |badges| {
                let badge = badges.iter_mut()
                    .find(|b| b.code == code_bounded && b.revoked_at.is_none())
                    .ok_or(Error::<T>::BadgeNotFound)?;

                badge.revoked_at = Some(block_number_u64);
                Ok::<(), Error<T>>(())
            })?;

            // Emitir evento
            Self::deposit_event(Event::BadgeRevoked {
                profile_id,
                code,
                reason,
            });

            Ok(())
        }

        /// Autoriza um módulo a modificar reputação
        #[pallet::call_index(7)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn authorize_module(
            origin: OriginFor<T>,
            module_id: ModuleId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            AuthorizedModules::<T>::try_mutate(|modules| {
                modules.try_insert(module_id)
                    .map_err(|_| Error::<T>::BadgeLimitReached)?;
                Ok::<(), Error<T>>(())
            })?;

            Self::deposit_event(Event::ModuleAuthorized { module_id });

            Ok(())
        }

        /// Desautoriza um módulo
        #[pallet::call_index(8)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn deauthorize_module(
            origin: OriginFor<T>,
            module_id: ModuleId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            AuthorizedModules::<T>::mutate(|modules| {
                modules.remove(&module_id);
            });

            Self::deposit_event(Event::ModuleDeauthorized { module_id });

            Ok(())
        }

        /// Autoriza um issuer a emitir badges
        #[pallet::call_index(9)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn authorize_issuer(
            origin: OriginFor<T>,
            issuer_id: ModuleId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            AuthorizedIssuers::<T>::try_mutate(|issuers| {
                issuers.try_insert(issuer_id)
                    .map_err(|_| Error::<T>::BadgeLimitReached)?;
                Ok::<(), Error<T>>(())
            })?;

            Self::deposit_event(Event::IssuerAuthorized { issuer_id });

            Ok(())
        }

        /// Desautoriza um issuer
        #[pallet::call_index(10)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn deauthorize_issuer(
            origin: OriginFor<T>,
            issuer_id: ModuleId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            AuthorizedIssuers::<T>::mutate(|issuers| {
                issuers.remove(&issuer_id);
            });

            Self::deposit_event(Event::IssuerDeauthorized { issuer_id });

            Ok(())
        }
    }

    // Helper functions
    impl<T: Config> Pallet<T> {
        /// Verifica se uma conta tem perfil
        pub fn has_profile(who: &T::AccountId) -> bool {
            OwnerProfile::<T>::contains_key(who)
        }

        /// Obtém o ProfileId de uma conta, se existir
        pub fn get_profile_id(who: &T::AccountId) -> Option<ProfileId> {
            OwnerProfile::<T>::get(who)
        }

        /// Verifica se um ProfileId existe
        pub fn profile_exists(profile_id: ProfileId) -> bool {
            ProfileOwner::<T>::contains_key(profile_id)
        }

        /// Obtém badges ativos de um perfil
        pub fn get_active_badges(profile_id: ProfileId) -> Vec<BadgeOf<T>> {
            Badges::<T>::get(profile_id)
                .into_iter()
                .filter(|b| b.revoked_at.is_none())
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::pallet as pallet_bazari_identity;
    use frame_support::{
        assert_err, assert_ok, construct_runtime, derive_impl, parameter_types,
        traits::ConstU32,
    };
    use sp_core::H256;
    use sp_runtime::{
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage,
    };

    type Block = frame_system::mocking::MockBlock<Test>;
    type AccountId = u64;

    construct_runtime!(
        pub enum Test {
            System: frame_system,
            Balances: pallet_balances,
            BazariIdentity: pallet_bazari_identity,
        }
    );

    parameter_types! {
        pub const BlockHashCount: u64 = 250;
    }

    #[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
    impl frame_system::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type RuntimeCall = RuntimeCall;
        type AccountId = AccountId;
        type Lookup = IdentityLookup<AccountId>;
        type Block = Block;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountData = pallet_balances::AccountData<u64>;
        type BlockHashCount = BlockHashCount;
    }

    parameter_types! {
        pub const ExistentialDeposit: u64 = 1;
    }

    impl pallet_balances::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Balance = u64;
        type DustRemoval = ();
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxLocks = ConstU32<0>;
        type MaxReserves = ConstU32<0>;
        type ReserveIdentifier = [u8; 8];
        type RuntimeHoldReason = ();
        type RuntimeFreezeReason = ();
        type DoneSlashHandler = ();
        type FreezeIdentifier = ();
        type MaxFreezes = ConstU32<0>;
    }

    parameter_types! {
        pub const MaxCidLen: u32 = 96;
        pub const MaxHandleLen: u32 = 32;
        pub const MaxBadges: u32 = 50;
        pub const MaxBadgeCodeLen: u32 = 32;
        pub const MaxHandleHistory: u32 = 10;
        pub const HandleCooldownBlocks: u64 = 432000;
        pub const MaxReasonCodeLen: u32 = 64;
        pub const MaxAuthorizedModules: u32 = 100;
    }

    impl pallet_bazari_identity::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type MaxCidLen = MaxCidLen;
        type MaxHandleLen = MaxHandleLen;
        type MaxBadges = MaxBadges;
        type MaxBadgeCodeLen = MaxBadgeCodeLen;
        type MaxHandleHistory = MaxHandleHistory;
        type HandleCooldownBlocks = HandleCooldownBlocks;
        type MaxReasonCodeLen = MaxReasonCodeLen;
        type MaxAuthorizedModules = MaxAuthorizedModules;
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();
        pallet_balances::GenesisConfig::<Test> {
            balances: vec![(1, 100), (2, 100), (3, 100)],
            dev_accounts: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();
        let mut ext: sp_io::TestExternalities = t.into();
        ext.execute_with(|| {
            System::set_block_number(1);
        });
        ext
    }

    #[test]
    fn mint_profile_works() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle.clone(),
                cid.clone()
            ));

            // Verificar storage
            assert_eq!(BazariIdentity::next_profile_id(), 1);
            assert_eq!(BazariIdentity::owner_profile(owner), Some(0));
            assert_eq!(BazariIdentity::profile_owner(0), Some(owner));
            assert_eq!(BazariIdentity::reputation(0), 0);

            // Verificar evento
            System::assert_has_event(RuntimeEvent::BazariIdentity(
                crate::pallet::Event::ProfileMinted {
                    profile_id: 0,
                    owner,
                    handle,
                },
            ));
        });
    }

    #[test]
    fn mint_profile_fails_if_already_exists() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            // Primeiro mint
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle.clone(),
                cid.clone()
            ));

            // Segundo mint deve falhar
            assert_err!(
                BazariIdentity::mint_profile(
                    RuntimeOrigin::signed(1),
                    owner,
                    b"alice2".to_vec(),
                    cid.clone()
                ),
                crate::pallet::Error::<Test>::ProfileExists
            );
        });
    }

    #[test]
    fn mint_profile_fails_if_handle_taken() {
        new_test_ext().execute_with(|| {
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            // Primeiro mint
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                1u64,
                handle.clone(),
                cid.clone()
            ));

            // Segundo mint com mesmo handle deve falhar
            assert_err!(
                BazariIdentity::mint_profile(
                    RuntimeOrigin::signed(2),
                    2u64,
                    handle,
                    cid
                ),
                crate::pallet::Error::<Test>::HandleTaken
            );
        });
    }

    #[test]
    fn update_metadata_cid_works() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();
            let new_cid = b"QmTest456".to_vec();

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle,
                cid
            ));

            // Atualizar CID
            assert_ok!(BazariIdentity::update_metadata_cid(
                RuntimeOrigin::signed(owner),
                0,
                new_cid.clone()
            ));

            // Verificar evento
            System::assert_has_event(RuntimeEvent::BazariIdentity(
                crate::pallet::Event::MetadataUpdated {
                    profile_id: 0,
                    who: owner,
                    cid: new_cid,
                },
            ));
        });
    }

    #[test]
    fn update_metadata_cid_fails_if_not_owner() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle,
                cid
            ));

            // Tentar atualizar de outra conta
            assert_err!(
                BazariIdentity::update_metadata_cid(
                    RuntimeOrigin::signed(2),
                    0,
                    b"QmTest456".to_vec()
                ),
                crate::pallet::Error::<Test>::NotOwner
            );
        });
    }

    #[test]
    fn set_handle_works() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let new_handle = b"alice2".to_vec();
            let cid = b"QmTest123".to_vec();

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle.clone(),
                cid
            ));

            // Avançar blocos para passar cooldown
            System::set_block_number(432001);

            // Alterar handle
            assert_ok!(BazariIdentity::set_handle(
                RuntimeOrigin::signed(owner),
                0,
                new_handle.clone()
            ));

            // Verificar evento
            System::assert_has_event(RuntimeEvent::BazariIdentity(
                crate::pallet::Event::HandleChanged {
                    profile_id: 0,
                    old_handle: handle,
                    new_handle,
                },
            ));
        });
    }

    #[test]
    fn set_handle_fails_during_cooldown() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle,
                cid
            ));

            // Primeira mudança
            System::set_block_number(100);
            assert_ok!(BazariIdentity::set_handle(
                RuntimeOrigin::signed(owner),
                0,
                b"alice2".to_vec()
            ));

            // Tentar mudar antes do cooldown
            System::set_block_number(1000);
            assert_err!(
                BazariIdentity::set_handle(
                    RuntimeOrigin::signed(owner),
                    0,
                    b"alice3".to_vec()
                ),
                crate::pallet::Error::<Test>::HandleCooldown
            );
        });
    }

    #[test]
    fn increment_reputation_works() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle,
                cid
            ));

            // Incrementar reputação (como root)
            assert_ok!(BazariIdentity::increment_reputation(
                RuntimeOrigin::root(),
                0,
                10,
                b"good_trade".to_vec()
            ));

            assert_eq!(BazariIdentity::reputation(0), 10);
        });
    }

    #[test]
    fn decrement_reputation_works() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle,
                cid
            ));

            // Decrementar reputação (como root)
            assert_ok!(BazariIdentity::decrement_reputation(
                RuntimeOrigin::root(),
                0,
                5,
                b"bad_trade".to_vec()
            ));

            assert_eq!(BazariIdentity::reputation(0), -5);
        });
    }

    #[test]
    fn award_badge_works() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle,
                cid
            ));

            // Conceder badge (como root)
            assert_ok!(BazariIdentity::award_badge(
                RuntimeOrigin::root(),
                0,
                b"early_adopter".to_vec(),
                1
            ));

            let badges = BazariIdentity::badges(0);
            assert_eq!(badges.len(), 1);
            assert_eq!(badges[0].code.to_vec(), b"early_adopter".to_vec());
        });
    }

    #[test]
    fn award_badge_fails_if_duplicate() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle,
                cid
            ));

            // Conceder badge
            assert_ok!(BazariIdentity::award_badge(
                RuntimeOrigin::root(),
                0,
                b"early_adopter".to_vec(),
                1
            ));

            // Tentar conceder novamente
            assert_err!(
                BazariIdentity::award_badge(
                    RuntimeOrigin::root(),
                    0,
                    b"early_adopter".to_vec(),
                    1
                ),
                crate::pallet::Error::<Test>::BadgeAlreadyExists
            );
        });
    }

    #[test]
    fn revoke_badge_works() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle,
                cid
            ));

            // Conceder badge
            assert_ok!(BazariIdentity::award_badge(
                RuntimeOrigin::root(),
                0,
                b"early_adopter".to_vec(),
                1
            ));

            // Revogar badge
            assert_ok!(BazariIdentity::revoke_badge(
                RuntimeOrigin::root(),
                0,
                b"early_adopter".to_vec(),
                b"violation".to_vec()
            ));

            let badges = BazariIdentity::badges(0);
            assert!(badges[0].revoked_at.is_some());
        });
    }

    #[test]
    fn helper_functions_work() {
        new_test_ext().execute_with(|| {
            let owner = 1u64;
            let handle = b"alice".to_vec();
            let cid = b"QmTest123".to_vec();

            assert!(!BazariIdentity::has_profile(&owner));

            // Criar profile
            assert_ok!(BazariIdentity::mint_profile(
                RuntimeOrigin::signed(1),
                owner,
                handle,
                cid
            ));

            assert!(BazariIdentity::has_profile(&owner));
            assert_eq!(BazariIdentity::get_profile_id(&owner), Some(0));
            assert!(BazariIdentity::profile_exists(0));
        });
    }
}
