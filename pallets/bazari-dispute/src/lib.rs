#![cfg_attr(not(feature = "std"), no_std)]

//! # Bazari Dispute Pallet
//!
//! Dispute resolution com VRF juror selection e commit-reveal voting.
//!
//! ## Overview
//!
//! - VRF: Random juror selection (5 jurors with reputation > 500)
//! - Commit-Reveal: 2-phase voting (24h commit, 24h reveal)
//! - Quorum: 3-of-5 majority required
//! - Ruling: RefundBuyer / ReleaseSeller / PartialRefund

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use codec::{Decode, Encode, MaxEncodedLen};
	use scale_info::TypeInfo;
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, Randomness},
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Hash;

	#[cfg(not(feature = "std"))]
	use alloc::vec::Vec;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Dispute status
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum DisputeStatus {
		/// Dispute created, awaiting juror selection
		Open,
		/// Jurors selected, awaiting commit phase
		JurorsSelected,
		/// In commit phase (jurors submit hashes)
		CommitPhase,
		/// In reveal phase (jurors reveal votes)
		RevealPhase,
		/// Dispute resolved
		Resolved,
	}

	/// Ruling outcome
	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum Ruling {
		/// Full refund to buyer
		RefundBuyer,
		/// Release escrow to seller
		ReleaseSeller,
		/// Partial refund (0-100%)
		PartialRefund(u8),
	}

	/// Vote choice (used in commit-reveal)
	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum Vote {
		RefundBuyer,
		ReleaseSeller,
		PartialRefund(u8),
	}

	// Manual implementations for DecodeWithMemTracking
	impl codec::DecodeWithMemTracking for Ruling {}
	impl codec::DecodeWithMemTracking for Vote {}
	impl codec::DecodeWithMemTracking for DisputeStatus {}

	/// Dispute data
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Dispute<T: Config> {
		pub dispute_id: u64,
		pub order_id: u64,
		pub plaintiff: T::AccountId,
		pub defendant: T::AccountId,
		pub jurors: BoundedVec<T::AccountId, ConstU32<5>>,
		pub evidence_cid: BoundedVec<u8, ConstU32<64>>,
		pub status: DisputeStatus,
		pub ruling: Option<Ruling>,
		pub created_at: BlockNumberFor<T>,
		pub commit_deadline: BlockNumberFor<T>,
		pub reveal_deadline: BlockNumberFor<T>,
	}

	/// Juror vote commitment
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct VoteCommit<T: Config> {
		pub commit_hash: T::Hash,
		pub revealed: bool,
	}

	/// Weight info trait
	pub trait WeightInfo {
		fn open_dispute() -> Weight;
		fn commit_vote() -> Weight;
		fn reveal_vote() -> Weight;
		fn execute_ruling() -> Weight;
	}

	impl WeightInfo for () {
		fn open_dispute() -> Weight {
			Weight::from_parts(100_000, 0)
		}
		fn commit_vote() -> Weight {
			Weight::from_parts(50_000, 0)
		}
		fn reveal_vote() -> Weight {
			Weight::from_parts(50_000, 0)
		}
		fn execute_ruling() -> Weight {
			Weight::from_parts(100_000, 0)
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config
		+ pallet_bazari_commerce::Config
		+ pallet_bazari_escrow::Config
		+ pallet_bazari_fulfillment::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Currency<Self::AccountId>;

		/// Randomness source for VRF juror selection
		type Randomness: Randomness<Self::Hash, BlockNumberFor<Self>>;

		/// Commit phase duration (blocks)
		#[pallet::constant]
		type CommitPhaseDuration: Get<BlockNumberFor<Self>>;

		/// Reveal phase duration (blocks)
		#[pallet::constant]
		type RevealPhaseDuration: Get<BlockNumberFor<Self>>;

		/// Minimum reputation for jurors
		#[pallet::constant]
		type MinJurorReputation: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	/// Next dispute ID
	#[pallet::storage]
	#[pallet::getter(fn next_dispute_id)]
	pub type NextDisputeId<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Disputes by ID
	#[pallet::storage]
	#[pallet::getter(fn disputes)]
	pub type Disputes<T: Config> = StorageMap<_, Blake2_128Concat, u64, Dispute<T>, OptionQuery>;

	/// Vote commits (dispute_id, juror) -> VoteCommit
	#[pallet::storage]
	pub type VoteCommits<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, u64,
		Blake2_128Concat, T::AccountId,
		VoteCommit<T>,
		OptionQuery,
	>;

	/// Revealed votes (dispute_id, juror) -> Vote
	#[pallet::storage]
	pub type RevealedVotes<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, u64,
		Blake2_128Concat, T::AccountId,
		Vote,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Dispute opened
		DisputeOpened {
			dispute_id: u64,
			order_id: u64,
			plaintiff: T::AccountId,
		},
		/// Jurors selected
		JurorsSelected {
			dispute_id: u64,
			jurors: Vec<T::AccountId>,
		},
		/// Vote committed
		VoteCommitted {
			dispute_id: u64,
			juror: T::AccountId,
		},
		/// Vote revealed
		VoteRevealed {
			dispute_id: u64,
			juror: T::AccountId,
		},
		/// Ruling executed
		RulingExecuted {
			dispute_id: u64,
			ruling: Ruling,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Order not found
		OrderNotFound,
		/// Dispute not found
		DisputeNotFound,
		/// Not authorized (not plaintiff/defendant)
		NotAuthorized,
		/// Dispute already exists for order
		DisputeAlreadyExists,
		/// Not in correct status
		InvalidStatus,
		/// Not a selected juror
		NotJuror,
		/// Commit phase ended
		CommitPhaseEnded,
		/// Reveal phase not started
		RevealPhaseNotStarted,
		/// Reveal phase ended
		RevealPhaseEnded,
		/// Vote not committed
		VoteNotCommitted,
		/// Invalid vote reveal (hash mismatch)
		InvalidVoteReveal,
		/// Not enough votes for quorum
		NotEnoughVotes,
		/// Not enough eligible jurors
		NotEnoughEligibleJurors,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Open a dispute for an order
		///
		/// # Arguments
		/// * `order_id` - Order to dispute
		/// * `evidence_cid` - IPFS CID with evidence
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::open_dispute())]
		pub fn open_dispute(
			origin: OriginFor<T>,
			order_id: u64,
			evidence_cid: Vec<u8>,
		) -> DispatchResult {
			let plaintiff = ensure_signed(origin)?;

			// Verify order exists
			ensure!(
				pallet_bazari_commerce::Orders::<T>::contains_key(order_id),
				Error::<T>::OrderNotFound
			);

			// Get dispute ID
			let dispute_id = NextDisputeId::<T>::get();
			let next_id = dispute_id.checked_add(1).ok_or(Error::<T>::DisputeAlreadyExists)?;

			let current_block = frame_system::Pallet::<T>::block_number();
			let commit_deadline = current_block + T::CommitPhaseDuration::get();
			let reveal_deadline = commit_deadline + T::RevealPhaseDuration::get();

			// Convert evidence_cid to BoundedVec
			let bounded_evidence: BoundedVec<u8, ConstU32<64>> =
				evidence_cid.try_into().map_err(|_| Error::<T>::DisputeAlreadyExists)?;

			// Select jurors using VRF
			let jurors = Self::select_jurors_vrf(dispute_id)?;

			// Create dispute
			let dispute = Dispute {
				dispute_id,
				order_id,
				plaintiff: plaintiff.clone(),
				defendant: plaintiff.clone(), // Will be set from order
				jurors: jurors.clone(),
				evidence_cid: bounded_evidence,
				status: DisputeStatus::CommitPhase,
				ruling: None,
				created_at: current_block,
				commit_deadline,
				reveal_deadline,
			};

			Disputes::<T>::insert(dispute_id, dispute);
			NextDisputeId::<T>::put(next_id);

			Self::deposit_event(Event::DisputeOpened {
				dispute_id,
				order_id,
				plaintiff: plaintiff.clone(),
			});

			Self::deposit_event(Event::JurorsSelected {
				dispute_id,
				jurors: jurors.into_inner(),
			});

			Ok(())
		}

		/// Commit a vote (hash of vote + salt)
		///
		/// # Arguments
		/// * `dispute_id` - Dispute ID
		/// * `commit_hash` - Hash of (vote + salt)
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::commit_vote())]
		pub fn commit_vote(
			origin: OriginFor<T>,
			dispute_id: u64,
			commit_hash: T::Hash,
		) -> DispatchResult {
			let juror = ensure_signed(origin)?;

			let dispute = Disputes::<T>::get(dispute_id).ok_or(Error::<T>::DisputeNotFound)?;

			// Verify juror is selected
			ensure!(dispute.jurors.contains(&juror), Error::<T>::NotJuror);

			// Verify we're in commit phase
			ensure!(dispute.status == DisputeStatus::CommitPhase, Error::<T>::InvalidStatus);

			let current_block = frame_system::Pallet::<T>::block_number();
			ensure!(current_block <= dispute.commit_deadline, Error::<T>::CommitPhaseEnded);

			// Store commit
			let commit = VoteCommit::<T> {
				commit_hash,
				revealed: false,
			};

			VoteCommits::<T>::insert(dispute_id, &juror, commit);

			Self::deposit_event(Event::VoteCommitted {
				dispute_id,
				juror,
			});

			Ok(())
		}

		/// Reveal a vote
		///
		/// # Arguments
		/// * `dispute_id` - Dispute ID
		/// * `vote` - Vote choice
		/// * `salt` - Salt used in commit
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::reveal_vote())]
		pub fn reveal_vote(
			origin: OriginFor<T>,
			dispute_id: u64,
			vote: Vote,
			salt: Vec<u8>,
		) -> DispatchResult {
			let juror = ensure_signed(origin)?;

			let mut dispute = Disputes::<T>::get(dispute_id).ok_or(Error::<T>::DisputeNotFound)?;

			// Verify juror is selected
			ensure!(dispute.jurors.contains(&juror), Error::<T>::NotJuror);

			// Verify we're past commit deadline
			let current_block = frame_system::Pallet::<T>::block_number();
			ensure!(current_block > dispute.commit_deadline, Error::<T>::RevealPhaseNotStarted);
			ensure!(current_block <= dispute.reveal_deadline, Error::<T>::RevealPhaseEnded);

			// Verify commit exists
			let commit = VoteCommits::<T>::get(dispute_id, &juror)
				.ok_or(Error::<T>::VoteNotCommitted)?;

			// Verify hash matches
			let vote_hash = Self::hash_vote(&vote, &salt);
			ensure!(vote_hash == commit.commit_hash, Error::<T>::InvalidVoteReveal);

			// Store revealed vote
			RevealedVotes::<T>::insert(dispute_id, &juror, vote);

			// Update commit as revealed
			VoteCommits::<T>::mutate(dispute_id, &juror, |maybe_commit| {
				if let Some(ref mut c) = maybe_commit {
					c.revealed = true;
				}
			});

			// Update dispute status to reveal phase if needed
			if dispute.status == DisputeStatus::CommitPhase {
				dispute.status = DisputeStatus::RevealPhase;
				Disputes::<T>::insert(dispute_id, dispute);
			}

			Self::deposit_event(Event::VoteRevealed {
				dispute_id,
				juror,
			});

			Ok(())
		}

		/// Execute ruling after reveal phase
		///
		/// # Arguments
		/// * `dispute_id` - Dispute ID
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::execute_ruling())]
		pub fn execute_ruling(
			origin: OriginFor<T>,
			dispute_id: u64,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let mut dispute = Disputes::<T>::get(dispute_id).ok_or(Error::<T>::DisputeNotFound)?;

			// Verify reveal phase ended
			let current_block = frame_system::Pallet::<T>::block_number();
			ensure!(current_block > dispute.reveal_deadline, Error::<T>::RevealPhaseNotStarted);

			// Tally votes
			let ruling = Self::tally_votes(dispute_id)?;

			// Update dispute
			dispute.ruling = Some(ruling.clone());
			dispute.status = DisputeStatus::Resolved;
			Disputes::<T>::insert(dispute_id, dispute);

			Self::deposit_event(Event::RulingExecuted {
				dispute_id,
				ruling,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Select jurors using VRF
		fn select_jurors_vrf(dispute_id: u64) -> Result<BoundedVec<T::AccountId, ConstU32<5>>, DispatchError> {
			// Get randomness
			let (randomness, _) = T::Randomness::random(&dispute_id.encode());

			// Get all couriers with reputation >= MinJurorReputation
			let mut eligible_jurors = Vec::new();

			for (account, courier) in pallet_bazari_fulfillment::Couriers::<T>::iter() {
				if courier.reputation_score >= T::MinJurorReputation::get() {
					eligible_jurors.push(account);
				}
			}

			ensure!(eligible_jurors.len() >= 5, Error::<T>::NotEnoughEligibleJurors);

			// Use randomness to shuffle and select 5 unique jurors
			let mut selected = Vec::new();
			let seed = randomness.as_ref();
			let mut remaining_jurors = eligible_jurors.clone();

			for i in 0..5 {
				// Simple deterministic selection based on randomness
				let index = (u32::from_be_bytes([
					seed[i * 4],
					seed[i * 4 + 1],
					seed[i * 4 + 2],
					seed[i * 4 + 3],
				]) as usize) % remaining_jurors.len();

				selected.push(remaining_jurors[index].clone());
				// Remove selected juror to ensure uniqueness
				remaining_jurors.remove(index);
			}

			selected.try_into().map_err(|_| Error::<T>::NotEnoughEligibleJurors.into())
		}

		/// Hash vote with salt for commit-reveal
		fn hash_vote(vote: &Vote, salt: &[u8]) -> T::Hash {
			let mut data = vote.encode();
			data.extend_from_slice(salt);
			T::Hashing::hash(&data)
		}

		/// Tally votes and determine ruling (3-of-5 quorum)
		fn tally_votes(dispute_id: u64) -> Result<Ruling, DispatchError> {
			let mut refund_count = 0u32;
			let mut release_count = 0u32;
			let mut partial_sum = 0u32;
			let mut partial_count = 0u32;
			let mut total_votes = 0u32;

			// Count votes
			for (_juror, vote) in RevealedVotes::<T>::iter_prefix(dispute_id) {
				total_votes += 1;
				match vote {
					Vote::RefundBuyer => refund_count += 1,
					Vote::ReleaseSeller => release_count += 1,
					Vote::PartialRefund(percent) => {
						partial_count += 1;
						partial_sum += percent as u32;
					}
				}
			}

			// Require at least 3 votes (quorum)
			ensure!(total_votes >= 3, Error::<T>::NotEnoughVotes);

			// Determine majority ruling
			if refund_count >= 3 {
				Ok(Ruling::RefundBuyer)
			} else if release_count >= 3 {
				Ok(Ruling::ReleaseSeller)
			} else if partial_count >= 3 {
				// Average partial refunds
				let avg_percent = (partial_sum / partial_count) as u8;
				Ok(Ruling::PartialRefund(avg_percent))
			} else {
				// Mixed votes - default to partial refund of 50%
				Ok(Ruling::PartialRefund(50))
			}
		}
	}
}
