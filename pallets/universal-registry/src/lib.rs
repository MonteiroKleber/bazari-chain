#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{pallet_prelude::*, traits::EnsureOrigin, BoundedVec};
    use frame_system::pallet_prelude::*;
    use scale_info::prelude::vec::Vec;

    type NamespaceOf<T> = BoundedVec<u8, <T as Config>::MaxNamespaceLen>;
    type HeadCidOf<T> = BoundedVec<u8, <T as Config>::MaxHeadCidLen>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Tamanho máximo (em bytes) suportado para o namespace.
        #[pallet::constant]
        type MaxNamespaceLen: Get<u32>;

        /// Tamanho máximo (em bytes) suportado para o CID/head.
        #[pallet::constant]
        type MaxHeadCidLen: Get<u32>;

        /// Origem autorizada a chamar o extrinsic `set_head`.
        type SetHeadOrigin: EnsureOrigin<OriginFor<Self>, Success = Self::AccountId>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    #[pallet::getter(fn head_by_namespace)]
    pub type HeadByNamespace<T: Config> =
        StorageMap<_, Blake2_128Concat, NamespaceOf<T>, HeadCidOf<T>, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// HEAD atualizado para um namespace.
        HeadUpdated {
            who: Option<T::AccountId>,
            namespace: Vec<u8>,
            cid: Vec<u8>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Namespace excede o limite configurado.
        NamespaceTooLong,
        /// CID excede o limite configurado.
        CidTooLong,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Atualiza o HEAD de um namespace específico.
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(0, 1))]
        pub fn set_head(origin: OriginFor<T>, namespace: Vec<u8>, cid: Vec<u8>) -> DispatchResult {
            let who = T::SetHeadOrigin::ensure_origin(origin)?;
            Self::do_set_head(&namespace, &cid)?;
            Self::deposit_event(Event::HeadUpdated {
                who: Some(who),
                namespace,
                cid,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Atualiza o HEAD para uso interno de outros pallets.
        pub fn set_head_for(namespace: &[u8], cid: &[u8]) -> Result<(), Error<T>> {
            let namespace_vec = namespace.to_vec();
            let cid_vec = cid.to_vec();
            Self::do_set_head(&namespace_vec, &cid_vec)?;
            Self::deposit_event(Event::HeadUpdated {
                who: None,
                namespace: namespace_vec,
                cid: cid_vec,
            });
            Ok(())
        }

        /// Recupera o HEAD armazenado para um namespace, se existir.
        pub fn head(namespace: &[u8]) -> Option<Vec<u8>> {
            let namespace_vec = namespace.to_vec();
            let namespace_bounded = NamespaceOf::<T>::try_from(namespace_vec).ok()?;
            HeadByNamespace::<T>::get(namespace_bounded).map(|head| head.to_vec())
        }

        fn do_set_head(namespace: &Vec<u8>, cid: &Vec<u8>) -> Result<(), Error<T>> {
            let namespace_bounded = NamespaceOf::<T>::try_from(namespace.clone())
                .map_err(|_| Error::<T>::NamespaceTooLong)?;
            let cid_bounded =
                HeadCidOf::<T>::try_from(cid.clone()).map_err(|_| Error::<T>::CidTooLong)?;
            HeadByNamespace::<T>::insert(namespace_bounded, cid_bounded);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::{assert_ok, construct_runtime, derive_impl, parameter_types};
    use sp_core::H256;
    use sp_runtime::{traits::{BlakeTwo256, IdentityLookup}, BuildStorage};

    type AccountId = u64;

    parameter_types! {
        pub const BlockHashCount: u64 = 250;
        pub const MaxNamespaceLen: u32 = 32;
        pub const MaxHeadCidLen: u32 = 64;
    }

    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;

    construct_runtime!(
        pub enum TestRuntime where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system,
            Balances: pallet_balances,
            UniversalRegistry: crate::{Pallet, Call, Storage, Event<T>},
        }
    );

    type Block = frame_system::mocking::MockBlock<TestRuntime>;
    type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

    #[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
    impl frame_system::Config for TestRuntime {
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

    impl pallet_balances::Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
        type Balance = u64;
        type DustRemoval = ();
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxLocks = frame_support::traits::ConstU32<0>;
        type MaxReserves = frame_support::traits::ConstU32<0>;
        type ReserveIdentifier = [u8; 8];
        type RuntimeHoldReason = ();
        type RuntimeFreezeReason = ();
        type DoneSlashHandler = ();
        type FreezeIdentifier = ();
        type MaxFreezes = frame_support::traits::ConstU32<0>;
    }

    type EnsureSigned<AccountId> = frame_system::EnsureSigned<AccountId>;

    impl Config for TestRuntime {
        type RuntimeEvent = RuntimeEvent;
        type MaxNamespaceLen = MaxNamespaceLen;
        type MaxHeadCidLen = MaxHeadCidLen;
        type SetHeadOrigin = EnsureSigned<AccountId>;
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<TestRuntime>::default()
            .build_storage()
            .unwrap();
        pallet_balances::GenesisConfig::<TestRuntime> {
            balances: vec![(1, 100)],
            ..Default::default()
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
    fn set_head_dispatchable_updates_storage() {
        new_test_ext().execute_with(|| {
            let namespace = b"stores/1".to_vec();
            let cid = b"bafy-test".to_vec();

            assert_ok!(UniversalRegistry::set_head(RuntimeOrigin::signed(1), namespace.clone(), cid.clone()));
            assert_eq!(UniversalRegistry::head(b"stores/1"), Some(cid.clone()));

            let events = System::events();
            assert_eq!(events.len(), 1);
            let event = &events[0].event;
            assert!(matches!(event,
                RuntimeEvent::UniversalRegistry(pallet::Event::HeadUpdated { who: Some(account), namespace: ev_ns, cid: ev_cid })
                if account == &1 && ev_ns == namespace.as_slice() && ev_cid == cid.as_slice()
            ));
        });
    }

    #[test]
    fn set_head_for_allows_internal_updates_without_origin() {
        new_test_ext().execute_with(|| {
            let namespace = b"stores/2";
            let cid = b"bafy-internal";

            assert_ok!(UniversalRegistry::set_head_for(namespace, cid));
            assert_eq!(UniversalRegistry::head(namespace), Some(cid.to_vec()));

            let events = System::events();
            assert_eq!(events.len(), 1);
            let event = &events[0].event;
            assert!(matches!(event,
                RuntimeEvent::UniversalRegistry(pallet::Event::HeadUpdated { who: None, namespace: ev_ns, cid: ev_cid })
                if ev_ns == namespace && ev_cid == cid
            ));
        });
    }

    #[test]
    fn namespace_too_long_fails() {
        new_test_ext().execute_with(|| {
            let namespace = vec![0u8; (MaxNamespaceLen::get() + 1) as usize];
            let cid = b"cid".to_vec();
            assert!(UniversalRegistry::set_head_for(&namespace, &cid).is_err());
            assert!(matches!(
                UniversalRegistry::set_head(RuntimeOrigin::signed(1), namespace, cid),
                Err(_)
            ));
        });
    }

    #[test]
    fn cid_too_long_fails() {
        new_test_ext().execute_with(|| {
            let namespace = b"stores/3".to_vec();
            let cid = vec![0u8; (MaxHeadCidLen::get() + 1) as usize];
            assert!(UniversalRegistry::set_head_for(&namespace, &cid).is_err());
            assert!(matches!(
                UniversalRegistry::set_head(RuntimeOrigin::signed(1), namespace, cid),
                Err(_)
            ));
        });
    }
}
