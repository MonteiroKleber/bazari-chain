#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(feature = "universal-registry")]
use pallet_universal_registry::Pallet as UniversalRegistry;
#[cfg(feature = "universal-registry")]
use core::convert::TryInto;
#[cfg(feature = "universal-registry")]
use alloc::format;
#[cfg(feature = "universal-registry")]
use alloc::vec::Vec;

#[frame_support::pallet]
pub mod pallet {
    use core::{
        convert::{TryFrom, TryInto},
        marker::PhantomData,
    };
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, EnsureOrigin, ReservableCurrency},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_uniques as uniques;
    use scale_info::prelude::vec::Vec;
    use codec::{Encode as ScaleEncode, Decode as ScaleDecode};
    #[cfg(feature = "universal-registry")]
    use crate::RegistryBridge;
    use sp_runtime::traits::{CheckedAdd, One, StaticLookup, Zero};

    type BalanceOf<T> = <<T as uniques::Config>::Currency as Currency<
        <T as frame_system::Config>::AccountId,
    >>::Balance;
    type CurrencyOf<T> = <T as uniques::Config>::Currency;

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Default, PartialEq, Eq, Debug)]
    pub struct ReputationStats {
        pub sales: u64,
        pub positive: u64,
        pub negative: u64,
        pub volume_planck: u128,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config + uniques::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Identificador sequencial das lojas.
        type StoreId: Parameter
            + Member
            + Copy
            + Default
            + MaxEncodedLen
            + CheckedAdd
            + One
            + TryInto<<Self as uniques::Config>::ItemId>
            + TryFrom<<Self as uniques::Config>::ItemId>;

        /// Máximo de bytes permitidos para o CID
        #[pallet::constant]
        type MaxCidLen: Get<u32>;

        /// Limite de operadores por loja
        #[pallet::constant]
        type MaxOperators: Get<u32>;

        /// Depósito reservado ao criar uma loja
        #[pallet::constant]
        type CreationDeposit: Get<BalanceOf<Self>>;

        /// Origem autorizada a ajustar reputação
        type ReputationOrigin: EnsureOrigin<OriginFor<Self>, Success = Self::AccountId>;

        /// Máximo de lojas indexadas por owner (para listagem eficiente)
        #[pallet::constant]
        type MaxStoresPerOwner: Get<u32>;

        #[cfg(feature = "universal-registry")]
        type Registry: RegistryBridge<Self>;
    }

    #[pallet::type_value]
    pub fn NextStoreIdDefault<T: Config>() -> T::StoreId {
        One::one()
    }

    /// Próximo identificador de loja (sequencial)
    #[pallet::storage]
    pub type NextStoreId<T: Config> =
        StorageValue<_, T::StoreId, ValueQuery, NextStoreIdDefault<T>>;

    #[pallet::type_value]
    pub fn NextCollectionIdU32Default<T: Config>() -> u32 { 1 }

    /// Próximo identificador de coleção (sequencial por runtime, em u32)
    #[pallet::storage]
    pub type NextCollectionIdU32<T: Config> =
        StorageValue<_, u32, ValueQuery, NextCollectionIdU32Default<T>>;

    /// Coleção associada a um owner (uma por conta)
    #[pallet::storage]
    pub type OwnerCollection<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        <T as uniques::Config>::CollectionId,
        OptionQuery,
    >;

    /// Coleção onde cada loja (item) foi mintada
    #[pallet::storage]
    pub type StoreCollection<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::StoreId,
        <T as uniques::Config>::CollectionId,
        OptionQuery,
    >;

    /// Índice de lojas por owner (para listagens)
    #[pallet::storage]
    pub type OwnerStores<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        BoundedVec<T::StoreId, <T as Config>::MaxStoresPerOwner>,
        ValueQuery,
    >;

    /// Metadado CID (bounded) por StoreId
    #[pallet::storage]
    pub type MetadataCid<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::StoreId,
        BoundedVec<u8, <T as Config>::MaxCidLen>,
        OptionQuery,
    >;

    /// Lista de operadores autorizados por loja
    #[pallet::storage]
    pub type Operators<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::StoreId,
        BoundedVec<T::AccountId, <T as Config>::MaxOperators>,
        ValueQuery,
    >;

    /// Transferência pendente aguardando aceite do novo owner
    #[pallet::storage]
    pub type PendingTransfer<T: Config> =
        StorageMap<_, Blake2_128Concat, T::StoreId, T::AccountId, OptionQuery>;

    /// Reputação agregada da loja
    #[pallet::storage]
    pub type Reputation<T: Config> =
        StorageMap<_, Blake2_128Concat, T::StoreId, ReputationStats, ValueQuery>;

    /// Depósito reservado ao criar a loja
    #[pallet::storage]
    pub type CreationDeposit<T: Config> =
        StorageMap<_, Blake2_128Concat, T::StoreId, BalanceOf<T>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        StoreCreated {
            owner: T::AccountId,
            store_id: T::StoreId,
            cid: Vec<u8>,
        },
        StoreMetadataUpdated {
            who: T::AccountId,
            store_id: T::StoreId,
            cid: Vec<u8>,
        },
        OperatorAdded {
            owner: T::AccountId,
            store_id: T::StoreId,
            operator: T::AccountId,
        },
        OperatorRemoved {
            owner: T::AccountId,
            store_id: T::StoreId,
            operator: T::AccountId,
        },
        StoreTransferBegun {
            owner: T::AccountId,
            store_id: T::StoreId,
            new_owner: T::AccountId,
        },
        StoreTransferred {
            old_owner: T::AccountId,
            new_owner: T::AccountId,
            store_id: T::StoreId,
        },
        StoreReputationUpdated {
            who: T::AccountId,
            store_id: T::StoreId,
            sales_delta: u64,
            positive_delta: u64,
            negative_delta: u64,
            volume_delta: u128,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// CID muito longo para o limite configurado
        CidTooLong,
        /// Store não encontrada
        StoreNotFound,
        /// Apenas o owner pode realizar a operação
        NotOwner,
        /// Incremento do identificador excedeu o limite do tipo
        NoAvailableStoreId,
        /// Falha ao converter StoreId em ItemId do uniques
        StoreIdConversionFailed,
        /// Owner sem saldo para reservar o depósito de criação
        InsufficientBalance,
        /// Operador já cadastrado
        OperatorAlreadyExists,
        /// Limite de operadores atingido
        OperatorLimitReached,
        /// Operador não cadastrado
        OperatorNotFound,
        /// Já existe transferência pendente
        TransferAlreadyPending,
        /// Não há transferência pendente
        NoPendingTransfer,
        /// Origin não corresponde ao destinatário configurado
        NotPendingRecipient,
        /// Owner tentou transferir para si mesmo
        CannotTransferToSelf,
        /// Limite de lojas por owner atingido
        StoresPerOwnerLimitReached,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Cria uma nova loja com `cid`, criando/reutilizando a coleção do owner e mintando o item nela.
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn create_store(origin: OriginFor<T>, cid: Vec<u8>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let bounded: BoundedVec<_, T::MaxCidLen> =
                cid.clone().try_into().map_err(|_| Error::<T>::CidTooLong)?;

            let store_id = NextStoreId::<T>::get();
            let next = store_id
                .checked_add(&One::one())
                .ok_or(Error::<T>::NoAvailableStoreId)?;
            let item_id: <T as uniques::Config>::ItemId = store_id
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::StoreIdConversionFailed)?;
            // Resolve ou cria a coleção do owner
            let collection = if let Some(cid_existing) = OwnerCollection::<T>::get(&who) {
                cid_existing
            } else {
                // aloca novo collection id (u32 → CollectionId via SCALE decode)
                let cid_u32 = NextCollectionIdU32::<T>::get();
                let next_cid_u32 = cid_u32.checked_add(1).ok_or(Error::<T>::NoAvailableStoreId)?;
                let mut bytes = ScaleEncode::encode(&cid_u32);
                let cid: <T as uniques::Config>::CollectionId =
                    ScaleDecode::decode(&mut &bytes[..])
                        .map_err(|_| Error::<T>::NoAvailableStoreId)?;
                // cria a coleção sob o próprio owner como admin
                uniques::Pallet::<T>::create(
                    frame_system::RawOrigin::Signed(who.clone()).into(),
                    cid.clone(),
                    <T as frame_system::Config>::Lookup::unlookup(who.clone()),
                )?;
                OwnerCollection::<T>::insert(&who, cid.clone());
                NextCollectionIdU32::<T>::put(next_cid_u32);
                cid
            };

            let deposit = T::CreationDeposit::get();
            if !deposit.is_zero() {
                CurrencyOf::<T>::reserve(&who, deposit)
                    .map_err(|_| Error::<T>::InsufficientBalance)?;
            }
            CreationDeposit::<T>::insert(store_id, deposit);

            uniques::Pallet::<T>::mint(
                frame_system::RawOrigin::Signed(who.clone()).into(),
                collection.clone(),
                item_id,
                <T as frame_system::Config>::Lookup::unlookup(who.clone()),
            )?;

            // Indexa coleção da loja e loja por owner
            StoreCollection::<T>::insert(store_id, collection.clone());
            let mut list = OwnerStores::<T>::get(&who);
            if list.try_push(store_id).is_err() {
                return Err(Error::<T>::StoresPerOwnerLimitReached.into());
            }
            OwnerStores::<T>::insert(&who, list);

            MetadataCid::<T>::insert(store_id, bounded);
            NextStoreId::<T>::put(next);

            Self::deposit_event(Event::StoreCreated {
                owner: who,
                store_id,
                cid,
            });
            Ok(())
        }

        /// Atualiza metadados da loja (owner ou operador)
        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn update_metadata(
            origin: OriginFor<T>,
            store_id: T::StoreId,
            cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_can_manage(&store_id, &who)?;

            let bounded: BoundedVec<_, T::MaxCidLen> =
                cid.clone().try_into().map_err(|_| Error::<T>::CidTooLong)?;

            MetadataCid::<T>::insert(store_id.clone(), bounded);

            #[cfg(feature = "universal-registry")]
            <T as Config>::Registry::set_head(&store_id, &cid);

            Self::deposit_event(Event::StoreMetadataUpdated { who, store_id, cid });
            Ok(())
        }

        /// Adiciona operador (apenas owner)
        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn add_operator(
            origin: OriginFor<T>,
            store_id: T::StoreId,
            operator: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let owner = Self::owner_of(&store_id)?;
            ensure!(owner == who, Error::<T>::NotOwner);

            let mut already_exists = false;
            let mut limit_reached = false;
            Operators::<T>::mutate(store_id.clone(), |ops| {
                if ops.iter().any(|op| op == &operator) {
                    already_exists = true;
                } else if ops.try_push(operator.clone()).is_err() {
                    limit_reached = true;
                }
            });
            if already_exists {
                return Err(Error::<T>::OperatorAlreadyExists.into());
            }
            if limit_reached {
                return Err(Error::<T>::OperatorLimitReached.into());
            }

            Self::deposit_event(Event::OperatorAdded {
                owner: who,
                store_id,
                operator,
            });
            Ok(())
        }

        /// Remove operador (apenas owner)
        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn remove_operator(
            origin: OriginFor<T>,
            store_id: T::StoreId,
            operator: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let owner = Self::owner_of(&store_id)?;
            ensure!(owner == who, Error::<T>::NotOwner);

            let mut found = false;
            Operators::<T>::mutate(store_id.clone(), |ops| {
                if let Some(pos) = ops.iter().position(|op| op == &operator) {
                    ops.swap_remove(pos);
                    found = true;
                }
            });
            ensure!(found, Error::<T>::OperatorNotFound);

            Self::deposit_event(Event::OperatorRemoved {
                owner: who,
                store_id,
                operator,
            });
            Ok(())
        }

        /// Inicia transferência em duas etapas (apenas owner)
        #[pallet::call_index(4)]
        #[pallet::weight(10_000)]
        pub fn begin_transfer(
            origin: OriginFor<T>,
            store_id: T::StoreId,
            new_owner: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let owner = Self::owner_of(&store_id)?;
            ensure!(owner == who, Error::<T>::NotOwner);

            ensure!(
                PendingTransfer::<T>::get(store_id.clone()).is_none(),
                Error::<T>::TransferAlreadyPending
            );
            ensure!(owner != new_owner, Error::<T>::CannotTransferToSelf);

            PendingTransfer::<T>::insert(store_id.clone(), new_owner.clone());

            Self::deposit_event(Event::StoreTransferBegun {
                owner: who,
                store_id,
                new_owner,
            });
            Ok(())
        }

        /// Aceita transferência pendente (apenas novo owner)
        #[pallet::call_index(5)]
        #[pallet::weight(10_000)]
        pub fn accept_transfer(origin: OriginFor<T>, store_id: T::StoreId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let pending =
                PendingTransfer::<T>::get(store_id.clone()).ok_or(Error::<T>::NoPendingTransfer)?;
            ensure!(pending == who, Error::<T>::NotPendingRecipient);

            let collection = StoreCollection::<T>::get(&store_id).ok_or(Error::<T>::StoreNotFound)?;
            let item_id = Self::store_item_id(&store_id)?;
            let old_owner = uniques::Pallet::<T>::owner(collection.clone(), item_id)
                .ok_or(Error::<T>::StoreNotFound)?;

            let deposit = CreationDeposit::<T>::get(store_id.clone());
            if !deposit.is_zero() {
                CurrencyOf::<T>::reserve(&who, deposit)
                    .map_err(|_| Error::<T>::InsufficientBalance)?;
                CurrencyOf::<T>::unreserve(&old_owner, deposit);
            }

            uniques::Pallet::<T>::transfer(
                frame_system::RawOrigin::Signed(old_owner.clone()).into(),
                collection,
                item_id,
                <T as frame_system::Config>::Lookup::unlookup(who.clone()),
            )?;

            PendingTransfer::<T>::remove(store_id.clone());
            Operators::<T>::remove(store_id.clone());

            // Atualiza índice OwnerStores
            OwnerStores::<T>::mutate(&old_owner, |stores| {
                if let Some(pos) = stores.iter().position(|s| s == &store_id) {
                    stores.swap_remove(pos);
                }
            });
            OwnerStores::<T>::mutate(&who, |stores| {
                let _ = stores.try_push(store_id.clone());
            });

            Self::deposit_event(Event::StoreTransferred {
                old_owner,
                new_owner: who,
                store_id,
            });
            Ok(())
        }

        /// Ajusta reputação agregada da loja
        #[pallet::call_index(6)]
        #[pallet::weight(10_000)]
        pub fn bump_reputation(
            origin: OriginFor<T>,
            store_id: T::StoreId,
            sales_delta: u64,
            positive_delta: u64,
            negative_delta: u64,
            volume_delta: u128,
        ) -> DispatchResult {
            let who = T::ReputationOrigin::ensure_origin(origin)?;

            Reputation::<T>::mutate(store_id.clone(), |rep| {
                rep.sales = rep.sales.saturating_add(sales_delta);
                rep.positive = rep.positive.saturating_add(positive_delta);
                rep.negative = rep.negative.saturating_add(negative_delta);
                rep.volume_planck = rep.volume_planck.saturating_add(volume_delta);
            });

            Self::deposit_event(Event::StoreReputationUpdated {
                who,
                store_id,
                sales_delta,
                positive_delta,
                negative_delta,
                volume_delta,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn store_item_id(
            store_id: &T::StoreId,
        ) -> Result<<T as uniques::Config>::ItemId, Error<T>> {
            store_id
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::StoreIdConversionFailed)
        }

        fn owner_of(store_id: &T::StoreId) -> Result<T::AccountId, Error<T>> {
            let collection = StoreCollection::<T>::get(store_id).ok_or(Error::<T>::StoreNotFound)?;
            let item_id = Self::store_item_id(store_id)?;
            uniques::Pallet::<T>::owner(collection, item_id).ok_or(Error::<T>::StoreNotFound)
        }

        fn ensure_can_manage(store_id: &T::StoreId, who: &T::AccountId) -> Result<(), Error<T>> {
            let owner = Self::owner_of(store_id)?;
            if owner == *who {
                return Ok(());
            }
            let ops = Operators::<T>::get(store_id.clone());
            if ops.iter().any(|op| op == who) {
                Ok(())
            } else {
                Err(Error::<T>::NotOwner)
            }
        }
    }
}

#[cfg(feature = "universal-registry")]
pub trait RegistryBridge<T: Config> {
    fn set_head(store_id: &T::StoreId, cid: &[u8]);
}

#[cfg(feature = "universal-registry")]
impl<T> RegistryBridge<T> for ()
where
    T: Config + pallet_universal_registry::Config,
{
    fn set_head(store_id: &T::StoreId, cid: &[u8]) {
        if let Some(namespace) = build_namespace::<T>(store_id) {
            let _ = UniversalRegistry::<T>::set_head_for(namespace.as_slice(), cid);
        }
    }
}

#[cfg(feature = "universal-registry")]
fn build_namespace<T>(store_id: &T::StoreId) -> Option<Vec<u8>>
where
    T: Config,
{
    let item_id: <T as pallet_uniques::Config>::ItemId = store_id.clone().try_into().ok()?;
    let mut namespace = b"stores/".to_vec();
    namespace.extend_from_slice(format!("{:?}", item_id).as_bytes());
    Some(namespace)
}


#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::{assert_noop, assert_ok, derive_impl, parameter_types, BoundedVec};
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
            #[cfg(feature = "universal-registry")]
            UniversalRegistry: pallet_universal_registry,
            Stores: crate::{Pallet, Call, Storage, Event<T>},
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
    }

    #[cfg(feature = "universal-registry")]
    mod registry_limits {
        use super::*;
        parameter_types! {
            pub const RegistryNamespaceLimit: u32 = 32;
            pub const RegistryCidLimit: u32 = 64;
        }
    }

    #[cfg(feature = "universal-registry")]
    use registry_limits::{RegistryCidLimit, RegistryNamespaceLimit};

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
        type MaxFreezes = frame_support::traits::ConstU32<0>;
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

    #[cfg(feature = "universal-registry")]
    impl pallet_universal_registry::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type MaxNamespaceLen = RegistryNamespaceLimit;
        type MaxHeadCidLen = RegistryCidLimit;
        type SetHeadOrigin = frame_system::EnsureSigned<AccountId>;
    }

    impl Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type StoreId = u64;
        type MaxCidLen = MaxCidLen;
        type MaxOperators = MaxOperators;
        type CreationDeposit = CreationDepositConst;
        type ReputationOrigin = frame_system::EnsureSigned<AccountId>;
        type MaxStoresPerOwner = MaxStoresPerOwner;
        #[cfg(feature = "universal-registry")]
        type Registry = ();
    }

    fn new_test_ext() -> TestExternalities {
        let mut t = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();
        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (1, 1_000_000_000_000),
                (2, 1_000_000_000_000),
                (99, 1_000_000_000_000),
            ],
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
    fn create_store_initialises_state() {
        new_test_ext().execute_with(|| {
            let cid = b"cid-123".to_vec();
            assert_eq!(NextStoreId::<Test>::get(), 1);

            assert_ok!(Stores::create_store(RuntimeOrigin::signed(1), cid.clone()));

            let expected = BoundedVec::<_, MaxCidLen>::try_from(cid.clone()).unwrap();
            assert_eq!(MetadataCid::<Test>::get(1).unwrap(), expected);
            assert_eq!(NextStoreId::<Test>::get(), 2);
            assert_eq!(CreationDeposit::<Test>::get(1), 0);

            System::assert_last_event(RuntimeEvent::Stores(Event::StoreCreated {
                owner: 1,
                store_id: 1,
                cid,
            }));
        });
    }

    #[cfg(feature = "universal-registry")]
    #[test]
    fn update_metadata_sets_registry_head() {
        new_test_ext().execute_with(|| {
            let cid = b"cid-123".to_vec();
            assert_ok!(Stores::create_store(RuntimeOrigin::signed(1), cid));

            let new_cid = b"registry-update".to_vec();
            assert_ok!(Stores::update_metadata(
                RuntimeOrigin::signed(1),
                1,
                new_cid.clone()
            ));

            assert_eq!(
                pallet_universal_registry::Pallet::<Test>::head(b"stores/1"),
                Some(new_cid)
            );
        });
    }

    #[cfg(feature = "universal-registry")]
    #[test]
    fn registry_failure_does_not_revert_update_metadata() {
        new_test_ext().execute_with(|| {
            let cid = b"cid-initial".to_vec();
            assert_ok!(Stores::create_store(RuntimeOrigin::signed(1), cid));

            let long_cid = vec![b'a'; (RegistryCidLimit::get() + 5) as usize];
            assert_ok!(Stores::update_metadata(
                RuntimeOrigin::signed(1),
                1,
                long_cid.clone()
            ));

            // Metadata stored even when registry rejects
            let stored = MetadataCid::<Test>::get(1).unwrap();
            assert_eq!(stored.to_vec(), long_cid);
            assert_eq!(
                pallet_universal_registry::Pallet::<Test>::head(b"stores/1"),
                None
            );
        });
    }

    #[test]
    fn operators_acl_allows_updates() {
        new_test_ext().execute_with(|| {
            assert_ok!(Stores::create_store(
                RuntimeOrigin::signed(1),
                b"cid".to_vec()
            ));
            assert_ok!(Stores::add_operator(RuntimeOrigin::signed(1), 1, 2));

            System::assert_last_event(RuntimeEvent::Stores(Event::OperatorAdded {
                owner: 1,
                store_id: 1,
                operator: 2,
            }));

            let new_cid = b"cid-operator".to_vec();
            assert_ok!(Stores::update_metadata(
                RuntimeOrigin::signed(2),
                1,
                new_cid.clone()
            ));
            let expected = BoundedVec::<_, MaxCidLen>::try_from(new_cid.clone()).unwrap();
            assert_eq!(MetadataCid::<Test>::get(1).unwrap(), expected);

            System::assert_last_event(RuntimeEvent::Stores(Event::StoreMetadataUpdated {
                who: 2,
                store_id: 1,
                cid: new_cid,
            }));

            assert_ok!(Stores::remove_operator(RuntimeOrigin::signed(1), 1, 2));
            assert_noop!(
                Stores::update_metadata(RuntimeOrigin::signed(2), 1, b"fail".to_vec()),
                Error::<Test>::NotOwner
            );
        });
    }

    #[test]
    fn begin_and_accept_transfer_flow() {
        new_test_ext().execute_with(|| {
            assert_ok!(Stores::create_store(
                RuntimeOrigin::signed(1),
                b"cid".to_vec()
            ));
            assert_ok!(Stores::add_operator(RuntimeOrigin::signed(1), 1, 3));

            assert_ok!(Stores::begin_transfer(RuntimeOrigin::signed(1), 1, 2));
            System::assert_last_event(RuntimeEvent::Stores(Event::StoreTransferBegun {
                owner: 1,
                store_id: 1,
                new_owner: 2,
            }));

            assert_noop!(
                Stores::accept_transfer(RuntimeOrigin::signed(3), 1),
                Error::<Test>::NotPendingRecipient
            );

            assert_ok!(Stores::accept_transfer(RuntimeOrigin::signed(2), 1));
            System::assert_last_event(RuntimeEvent::Stores(Event::StoreTransferred {
                old_owner: 1,
                new_owner: 2,
                store_id: 1,
            }));

            let coll = StoreCollection::<Test>::get(1).expect("store collection must exist");
            let owner = pallet_uniques::Pallet::<Test>::owner(coll, 1).unwrap();
            assert_eq!(owner, 2);
            assert!(PendingTransfer::<Test>::get(1).is_none());
            assert!(Operators::<Test>::get(1).is_empty());
        });
    }

    #[test]
    fn bump_reputation_updates_counters() {
        new_test_ext().execute_with(|| {
            assert_ok!(Stores::create_store(
                RuntimeOrigin::signed(1),
                b"cid".to_vec()
            ));

            assert_ok!(Stores::bump_reputation(
                RuntimeOrigin::signed(99),
                1,
                5,
                4,
                1,
                10,
            ));

            let rep = Reputation::<Test>::get(1);
            assert_eq!(rep.sales, 5);
            assert_eq!(rep.positive, 4);
            assert_eq!(rep.negative, 1);
            assert_eq!(rep.volume_planck, 10);

            System::assert_last_event(RuntimeEvent::Stores(Event::StoreReputationUpdated {
                who: 99,
                store_id: 1,
                sales_delta: 5,
                positive_delta: 4,
                negative_delta: 1,
                volume_delta: 10,
            }));
        });
    }

    #[test]
    fn operator_errors_are_emitted() {
        new_test_ext().execute_with(|| {
            assert_ok!(Stores::create_store(
                RuntimeOrigin::signed(1),
                b"cid".to_vec()
            ));
            assert_ok!(Stores::add_operator(RuntimeOrigin::signed(1), 1, 2));
            assert_noop!(
                Stores::add_operator(RuntimeOrigin::signed(1), 1, 2),
                Error::<Test>::OperatorAlreadyExists
            );

            assert_ok!(Stores::add_operator(RuntimeOrigin::signed(1), 1, 3));
            assert_ok!(Stores::add_operator(RuntimeOrigin::signed(1), 1, 4));
            assert_noop!(
                Stores::add_operator(RuntimeOrigin::signed(1), 1, 5),
                Error::<Test>::OperatorLimitReached
            );

            assert_noop!(
                Stores::remove_operator(RuntimeOrigin::signed(1), 1, 9),
                Error::<Test>::OperatorNotFound
            );
        });
    }

    #[test]
    fn begin_transfer_requires_owner() {
        new_test_ext().execute_with(|| {
            assert_ok!(Stores::create_store(
                RuntimeOrigin::signed(1),
                b"cid".to_vec()
            ));
            assert_noop!(
                Stores::begin_transfer(RuntimeOrigin::signed(2), 1, 2),
                Error::<Test>::NotOwner
            );
        });
    }
}
