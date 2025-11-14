#![cfg_attr(not(feature = "std"), no_std)]

//! # Bazari Attestation Pallet
//!
//! Pallet for anchoring cryptographic proofs of handoff and delivery on-chain.
//!
//! ## Overview
//!
//! This pallet provides:
//! - **Proof Registry**: Immutable storage for HandoffProof and DeliveryProof
//! - **Co-signatures**: N-of-M quorum verification (both parties must sign)
//! - **IPFS Integration**: Store photos, GPS, timestamps off-chain
//! - **On-chain Validation**: Quorum verification + timestamps
//!
//! ## Interface
//!
//! ### Extrinsics
//!
//! - `submit_proof` - Submit new attestation (HandoffProof or DeliveryProof)
//! - `co_sign` - Co-sign existing attestation
//!
//! ### Storage
//!
//! - `Attestations` - Proof registry (attestation_id → Attestation)
//! - `OrderAttestations` - Order proofs index (order_id + proof_type → attestation_id)
//! - `AttestationIdCounter` - Auto-increment counter

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// Weight information for pallet_bazari_attestation
pub mod weights {
	use frame_support::weights::Weight;

	pub trait WeightInfo {
		fn submit_proof() -> Weight;
		fn co_sign() -> Weight;
	}

	impl WeightInfo for () {
		fn submit_proof() -> Weight {
			Weight::from_parts(10_000, 0)
		}
		fn co_sign() -> Weight {
			Weight::from_parts(10_000, 0)
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use serde::{Deserialize, Serialize};

	/// Proof type enumeration
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Serialize, Deserialize)]
	pub enum ProofType {
		/// Seller → Courier handoff proof
		HandoffProof,
		/// Courier → Buyer delivery proof
		DeliveryProof,
		/// Custom proof type (extensible)
		CustomProof,
	}

	/// Attestation struct
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct Attestation<T: Config> {
		/// Unique attestation ID
		pub attestation_id: u64,
		/// Order ID (FK to bazari-commerce)
		pub order_id: u64,
		/// Proof type
		pub proof_type: ProofType,
		/// IPFS CID (Qm... or bafybei...)
		pub ipfs_cid: BoundedVec<u8, ConstU32<64>>,
		/// Required signers (accounts that must sign)
		pub required_signers: BoundedVec<T::AccountId, ConstU32<10>>,
		/// Signatures collected so far
		pub signatures: BoundedVec<T::AccountId, ConstU32<10>>,
		/// Quorum threshold (N-of-M)
		pub quorum: u32,
		/// Is verified (quorum reached)
		pub verified: bool,
		/// Submission timestamp
		pub submitted_at: BlockNumberFor<T>,
		/// Verification timestamp
		pub verified_at: Option<BlockNumberFor<T>>,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_bazari_commerce::Config {
		/// The overarching event type
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Maximum number of signers per attestation
		#[pallet::constant]
		type MaxSigners: Get<u32>;

		/// Maximum IPFS CID length
		#[pallet::constant]
		type MaxCidLength: Get<u32>;

		/// Weight info (placeholder)
		type WeightInfo: crate::weights::WeightInfo;
	}

	// ============================================
	// STORAGE
	// ============================================

	/// Attestations registry
	#[pallet::storage]
	#[pallet::getter(fn attestations)]
	pub type Attestations<T: Config> = StorageMap<_, Blake2_128Concat, u64, Attestation<T>>;

	/// Order attestations index (order_id + proof_type → attestation_id)
	#[pallet::storage]
	#[pallet::getter(fn order_attestations)]
	pub type OrderAttestations<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		u64, // order_id
		Blake2_128Concat,
		ProofType,
		u64, // attestation_id
	>;

	/// Attestation ID counter
	#[pallet::storage]
	#[pallet::getter(fn attestation_id_counter)]
	pub type AttestationIdCounter<T: Config> = StorageValue<_, u64, ValueQuery>;

	// ============================================
	// EVENTS
	// ============================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Proof submitted
		ProofSubmitted {
			attestation_id: u64,
			order_id: u64,
			submitter: T::AccountId,
		},
		/// Proof co-signed
		ProofCoSigned {
			attestation_id: u64,
			signer: T::AccountId,
		},
		/// Proof verified (quorum reached)
		ProofVerified {
			attestation_id: u64,
			order_id: u64,
		},
	}

	// ============================================
	// ERRORS
	// ============================================

	#[pallet::error]
	pub enum Error<T> {
		/// Order not found
		OrderNotFound,
		/// Attestation not found
		AttestationNotFound,
		/// Unauthorized (not in required_signers)
		Unauthorized,
		/// Already signed
		AlreadySigned,
		/// Invalid quorum
		InvalidQuorum,
		/// CID too long
		CidTooLong,
		/// Too many signers
		TooManySigners,
		/// Too many signatures
		TooManySignatures,
		/// Invalid proof type
		InvalidProofType,
	}

	// ============================================
	// EXTRINSICS
	// ============================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Submit new proof
		///
		/// - `order_id`: Order ID (must exist in bazari-commerce)
		/// - `proof_type_id`: Proof type (0=HandoffProof, 1=DeliveryProof, 2=CustomProof)
		/// - `ipfs_cid`: IPFS CID of the proof data
		/// - `required_signers`: Accounts that must sign
		/// - `quorum`: N-of-M threshold
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::submit_proof())]
		pub fn submit_proof(
			origin: OriginFor<T>,
			order_id: u64,
			proof_type_id: u8,
			ipfs_cid: Vec<u8>,
			required_signers: Vec<T::AccountId>,
			quorum: u32,
		) -> DispatchResult {
			let submitter = ensure_signed(origin)?;

			// Validate order exists
			ensure!(
				pallet_bazari_commerce::Orders::<T>::contains_key(order_id),
				Error::<T>::OrderNotFound
			);

			// Validate submitter is in required_signers
			ensure!(
				required_signers.contains(&submitter),
				Error::<T>::Unauthorized
			);

			// Validate quorum
			ensure!(
				quorum > 0 && quorum <= required_signers.len() as u32,
				Error::<T>::InvalidQuorum
			);

			// Parse proof type from u8
			let proof_type = match proof_type_id {
				0 => ProofType::HandoffProof,
				1 => ProofType::DeliveryProof,
				2 => ProofType::CustomProof,
				_ => return Err(Error::<T>::InvalidProofType.into()),
			};

			let attestation_id = AttestationIdCounter::<T>::get();
			AttestationIdCounter::<T>::put(attestation_id.saturating_add(1));

			let bounded_cid: BoundedVec<u8, ConstU32<64>> =
				ipfs_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

			let bounded_signers: BoundedVec<T::AccountId, ConstU32<10>> =
				required_signers.try_into().map_err(|_| Error::<T>::TooManySigners)?;

			let attestation = Attestation {
				attestation_id,
				order_id,
				proof_type: proof_type.clone(),
				ipfs_cid: bounded_cid,
				required_signers: bounded_signers,
				signatures: BoundedVec::default(),
				quorum,
				verified: false,
				submitted_at: frame_system::Pallet::<T>::block_number(),
				verified_at: None,
			};

			Attestations::<T>::insert(attestation_id, attestation);
			OrderAttestations::<T>::insert(order_id, proof_type, attestation_id);

			Self::deposit_event(Event::ProofSubmitted {
				attestation_id,
				order_id,
				submitter,
			});

			Ok(())
		}

		/// Co-sign existing attestation
		///
		/// - `attestation_id`: Attestation ID to sign
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::co_sign())]
		pub fn co_sign(origin: OriginFor<T>, attestation_id: u64) -> DispatchResult {
			let signer = ensure_signed(origin)?;

			Attestations::<T>::try_mutate(attestation_id, |maybe_attestation| {
				let attestation = maybe_attestation
					.as_mut()
					.ok_or(Error::<T>::AttestationNotFound)?;

				// Validate signer is in required_signers
				ensure!(
					attestation.required_signers.contains(&signer),
					Error::<T>::Unauthorized
				);

				// Validate not already signed
				ensure!(
					!attestation.signatures.contains(&signer),
					Error::<T>::AlreadySigned
				);

				// Add signature
				attestation
					.signatures
					.try_push(signer.clone())
					.map_err(|_| Error::<T>::TooManySignatures)?;

				// Check if quorum reached
				if attestation.signatures.len() as u32 >= attestation.quorum {
					attestation.verified = true;
					attestation.verified_at =
						Some(frame_system::Pallet::<T>::block_number());

					Self::deposit_event(Event::ProofVerified {
						attestation_id,
						order_id: attestation.order_id,
					});
				}

				Self::deposit_event(Event::ProofCoSigned {
					attestation_id,
					signer,
				});

				Ok(())
			})
		}
	}

	// ============================================
	// HELPER FUNCTIONS
	// ============================================

	impl<T: Config> Pallet<T> {
		/// Verify if proof is verified (quorum reached)
		pub fn verify_proof(attestation_id: u64) -> bool {
			if let Some(attestation) = Attestations::<T>::get(attestation_id) {
				attestation.verified
			} else {
				false
			}
		}

		/// Get all proofs for an order
		pub fn get_order_proofs(order_id: u64) -> Vec<(ProofType, u64)> {
			let mut proofs = Vec::new();

			if let Some(handoff_id) =
				OrderAttestations::<T>::get(order_id, ProofType::HandoffProof)
			{
				proofs.push((ProofType::HandoffProof, handoff_id));
			}

			if let Some(delivery_id) =
				OrderAttestations::<T>::get(order_id, ProofType::DeliveryProof)
			{
				proofs.push((ProofType::DeliveryProof, delivery_id));
			}

			if let Some(custom_id) =
				OrderAttestations::<T>::get(order_id, ProofType::CustomProof)
			{
				proofs.push((ProofType::CustomProof, custom_id));
			}

			proofs
		}
	}
}
