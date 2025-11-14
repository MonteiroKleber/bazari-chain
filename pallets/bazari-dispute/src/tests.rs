use crate::{mock::*, Error, Event, Vote, DisputeStatus};
use frame_support::{assert_noop, assert_ok};
use codec::Encode;
use sp_runtime::traits::Hash;

fn setup_jurors() {
	// Register 10 couriers with high reputation to be eligible as jurors
	for i in 1..=10 {
		// Register identity first
		assert_ok!(BazariIdentity::mint_profile(
			RuntimeOrigin::signed(account(1)), // Root account minting
			account(i), // Owner
			format!("juror{}", i).as_bytes().to_vec(), // Handle
			b"ipfs://Qm...".to_vec() // CID
		));

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(account(i)),
			1000,
			vec![1, 2, 3]
		));

		// Set high reputation (>=500)
		pallet_bazari_fulfillment::Couriers::<Test>::mutate(account(i), |maybe_courier| {
			if let Some(ref mut courier) = maybe_courier {
				courier.reputation_score = 800;
			}
		});
	}
}

fn create_test_order(buyer: AccountId, seller: AccountId) -> u64 {
	assert_ok!(BazariCommerce::create_order(
		RuntimeOrigin::signed(buyer),
		0, // Marketplace
		None,
		seller,
		None,
		vec![(None, b"Test Item".to_vec(), 1, 10_000)]
	));
	0 // First order ID
}

#[test]
fn open_dispute_works() {
	new_test_ext().execute_with(|| {
		setup_jurors();
		let buyer = account(20);
		let seller = account(21);
		let order_id = create_test_order(buyer, seller);

		let evidence = b"ipfs://Qm...evidence".to_vec();

		assert_ok!(BazariDispute::open_dispute(
			RuntimeOrigin::signed(buyer),
			order_id,
			evidence.clone()
		));

		let dispute = BazariDispute::disputes(0).unwrap();
		assert_eq!(dispute.order_id, order_id);
		assert_eq!(dispute.plaintiff, buyer);
		assert_eq!(dispute.jurors.len(), 5);
		assert_eq!(dispute.status, DisputeStatus::CommitPhase);
	});
}

#[test]
fn open_dispute_fails_order_not_found() {
	new_test_ext().execute_with(|| {
		setup_jurors();
		let buyer = account(20);
		let evidence = b"ipfs://Qm...evidence".to_vec();

		assert_noop!(
			BazariDispute::open_dispute(
				RuntimeOrigin::signed(buyer),
				999, // Non-existent order
				evidence
			),
			Error::<Test>::OrderNotFound
		);
	});
}

#[test]
fn commit_vote_works() {
	new_test_ext().execute_with(|| {
		setup_jurors();
		let buyer = account(20);
		let seller = account(21);
		let order_id = create_test_order(buyer, seller);

		let evidence = b"ipfs://Qm...evidence".to_vec();
		assert_ok!(BazariDispute::open_dispute(
			RuntimeOrigin::signed(buyer),
			order_id,
			evidence
		));

		let dispute = BazariDispute::disputes(0).unwrap();
		let juror = dispute.jurors[0].clone();

		// Create vote hash
		let vote = Vote::RefundBuyer;
		let salt = b"secret_salt".to_vec();
		let vote_hash = <Test as frame_system::Config>::Hashing::hash(&{
			let mut data = vote.encode();
			data.extend_from_slice(&salt);
			data
		});

		assert_ok!(BazariDispute::commit_vote(
			RuntimeOrigin::signed(juror),
			0,
			vote_hash
		));
	});
}

#[test]
fn commit_vote_fails_not_juror() {
	new_test_ext().execute_with(|| {
		setup_jurors();
		let buyer = account(20);
		let seller = account(21);
		let order_id = create_test_order(buyer, seller);

		let evidence = b"ipfs://Qm...evidence".to_vec();
		assert_ok!(BazariDispute::open_dispute(
			RuntimeOrigin::signed(buyer),
			order_id,
			evidence
		));

		let non_juror = account(99);
		let vote_hash = <Test as frame_system::Config>::Hashing::hash(b"random_hash");

		assert_noop!(
			BazariDispute::commit_vote(
				RuntimeOrigin::signed(non_juror),
				0,
				vote_hash
			),
			Error::<Test>::NotJuror
		);
	});
}

#[test]
fn reveal_vote_works() {
	new_test_ext().execute_with(|| {
		setup_jurors();
		let buyer = account(20);
		let seller = account(21);
		let order_id = create_test_order(buyer, seller);

		let evidence = b"ipfs://Qm...evidence".to_vec();
		assert_ok!(BazariDispute::open_dispute(
			RuntimeOrigin::signed(buyer),
			order_id,
			evidence
		));

		let dispute = BazariDispute::disputes(0).unwrap();
		let juror = dispute.jurors[0].clone();

		// Commit vote
		let vote = Vote::RefundBuyer;
		let salt = b"secret_salt".to_vec();
		let vote_hash = <Test as frame_system::Config>::Hashing::hash(&{
			let mut data = vote.clone().encode();
			data.extend_from_slice(&salt);
			data
		});

		assert_ok!(BazariDispute::commit_vote(
			RuntimeOrigin::signed(juror),
			0,
			vote_hash
		));

		// Advance past commit deadline (commit_deadline is 1 + 100 = 101, so need > 101)
		System::set_block_number(102);

		// Reveal vote
		assert_ok!(BazariDispute::reveal_vote(
			RuntimeOrigin::signed(juror),
			0,
			vote,
			salt
		));
	});
}

#[test]
fn reveal_vote_fails_before_deadline() {
	new_test_ext().execute_with(|| {
		setup_jurors();
		let buyer = account(20);
		let seller = account(21);
		let order_id = create_test_order(buyer, seller);

		let evidence = b"ipfs://Qm...evidence".to_vec();
		assert_ok!(BazariDispute::open_dispute(
			RuntimeOrigin::signed(buyer),
			order_id,
			evidence
		));

		let dispute = BazariDispute::disputes(0).unwrap();
		let juror = dispute.jurors[0].clone();

		let vote = Vote::RefundBuyer;
		let salt = b"secret_salt".to_vec();
		let vote_hash = <Test as frame_system::Config>::Hashing::hash(&{
			let mut data = vote.clone().encode();
			data.extend_from_slice(&salt);
			data
		});

		assert_ok!(BazariDispute::commit_vote(
			RuntimeOrigin::signed(juror),
			0,
			vote_hash
		));

		// Try to reveal before deadline (still in commit phase)
		assert_noop!(
			BazariDispute::reveal_vote(
				RuntimeOrigin::signed(juror),
				0,
				vote,
				salt
			),
			Error::<Test>::RevealPhaseNotStarted
		);
	});
}

#[test]
fn reveal_vote_fails_invalid_hash() {
	new_test_ext().execute_with(|| {
		setup_jurors();
		let buyer = account(20);
		let seller = account(21);
		let order_id = create_test_order(buyer, seller);

		let evidence = b"ipfs://Qm...evidence".to_vec();
		assert_ok!(BazariDispute::open_dispute(
			RuntimeOrigin::signed(buyer),
			order_id,
			evidence
		));

		let dispute = BazariDispute::disputes(0).unwrap();
		let juror = dispute.jurors[0].clone();

		let vote = Vote::RefundBuyer;
		let salt = b"secret_salt".to_vec();
		let vote_hash = <Test as frame_system::Config>::Hashing::hash(&{
			let mut data = vote.clone().encode();
			data.extend_from_slice(&salt);
			data
		});

		assert_ok!(BazariDispute::commit_vote(
			RuntimeOrigin::signed(juror),
			0,
			vote_hash
		));

		System::set_block_number(102);

		// Try to reveal with wrong salt
		let wrong_salt = b"wrong_salt".to_vec();
		assert_noop!(
			BazariDispute::reveal_vote(
				RuntimeOrigin::signed(juror),
				0,
				vote,
				wrong_salt
			),
			Error::<Test>::InvalidVoteReveal
		);
	});
}

#[test]
fn execute_ruling_works() {
	new_test_ext().execute_with(|| {
		setup_jurors();
		let buyer = account(20);
		let seller = account(21);
		let order_id = create_test_order(buyer, seller);

		let evidence = b"ipfs://Qm...evidence".to_vec();
		assert_ok!(BazariDispute::open_dispute(
			RuntimeOrigin::signed(buyer),
			order_id,
			evidence
		));

		let dispute = BazariDispute::disputes(0).unwrap();

		// Commit votes from 3 jurors
		let juror0 = dispute.jurors[0].clone();
		let vote0 = Vote::RefundBuyer;
		let salt0 = b"salt_0".to_vec();
		let vote_hash0 = <Test as frame_system::Config>::Hashing::hash(&{
			let mut data = vote0.clone().encode();
			data.extend_from_slice(&salt0);
			data
		});
		assert_ok!(BazariDispute::commit_vote(
			RuntimeOrigin::signed(juror0),
			0,
			vote_hash0
		));

		let juror1 = dispute.jurors[1].clone();
		let vote1 = Vote::RefundBuyer;
		let salt1 = b"salt_1".to_vec();
		let vote_hash1 = <Test as frame_system::Config>::Hashing::hash(&{
			let mut data = vote1.clone().encode();
			data.extend_from_slice(&salt1);
			data
		});
		assert_ok!(BazariDispute::commit_vote(
			RuntimeOrigin::signed(juror1),
			0,
			vote_hash1
		));

		let juror2 = dispute.jurors[2].clone();
		let vote2 = Vote::RefundBuyer;
		let salt2 = b"salt_2".to_vec();
		let vote_hash2 = <Test as frame_system::Config>::Hashing::hash(&{
			let mut data = vote2.clone().encode();
			data.extend_from_slice(&salt2);
			data
		});
		assert_ok!(BazariDispute::commit_vote(
			RuntimeOrigin::signed(juror2),
			0,
			vote_hash2
		));

		// Advance past commit deadline (commit_deadline is 1 + 100 = 101, so need > 101)
		System::set_block_number(102);

		// Reveal votes
		assert_ok!(BazariDispute::reveal_vote(
			RuntimeOrigin::signed(juror0),
			0,
			vote0,
			salt0
		));

		assert_ok!(BazariDispute::reveal_vote(
			RuntimeOrigin::signed(juror1),
			0,
			vote1,
			salt1
		));

		assert_ok!(BazariDispute::reveal_vote(
			RuntimeOrigin::signed(juror2),
			0,
			vote2,
			salt2
		));

		// Advance past reveal deadline (reveal_deadline is 101 + 100 = 201, so need > 201)
		System::set_block_number(202);

		// Execute ruling
		assert_ok!(BazariDispute::execute_ruling(
			RuntimeOrigin::signed(buyer),
			0
		));

		let final_dispute = BazariDispute::disputes(0).unwrap();
		assert_eq!(final_dispute.status, DisputeStatus::Resolved);
		assert!(final_dispute.ruling.is_some());
	});
}
