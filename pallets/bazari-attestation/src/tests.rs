use crate::{mock::*, Error, Event, ProofType};
use frame_support::{assert_noop, assert_ok};

// Helper function to create an order
fn create_test_order(buyer: AccountId, seller: AccountId) -> u64 {
	// Create a simple order item as tuple: (product_id, name, quantity, unit_price)
	let item = (
		Some(1), // product_id
		b"Test Product".to_vec(), // name
		1, // quantity
		1000, // unit_price
	);

	assert_ok!(BazariCommerce::create_order(
		RuntimeOrigin::signed(buyer),
		0, // source (Marketplace)
		None, // thread_id
		seller,
		Some(1), // store_id
		vec![item], // items
	));
	// Order ID starts at 0
	0
}

#[test]
fn submit_handoff_proof_works() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);
		let buyer = account(3);

		// Create order first
		let order_id = create_test_order(buyer, seller);

		// Seller submits HandoffProof
		assert_ok!(BazariAttestation::submit_proof(
			RuntimeOrigin::signed(seller),
			order_id,
			0, // HandoffProof
			b"QmHandoffPhoto123".to_vec(), // IPFS CID
			vec![seller, courier], // 2 signers
			2, // 2-of-2 quorum
		));

		let attestation_id = 0;

		// Verify attestation created
		let attestation = BazariAttestation::attestations(attestation_id).unwrap();
		assert_eq!(attestation.attestation_id, 0);
		assert_eq!(attestation.order_id, order_id);
		assert_eq!(attestation.proof_type, ProofType::HandoffProof);
		assert_eq!(attestation.ipfs_cid.to_vec(), b"QmHandoffPhoto123".to_vec());
		assert_eq!(attestation.quorum, 2);
		assert!(!attestation.verified);
		assert_eq!(attestation.signatures.len(), 0);

		// Verify event emitted
		System::assert_last_event(
			Event::ProofSubmitted {
				attestation_id,
				order_id,
				submitter: seller,
			}
			.into(),
		);
	});
}

#[test]
fn handoff_proof_2_of_2_quorum_works() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);
		let buyer = account(3);

		// Create order
		let order_id = create_test_order(buyer, seller);

		// Seller submits HandoffProof
		assert_ok!(BazariAttestation::submit_proof(
			RuntimeOrigin::signed(seller),
			order_id,
			0, // HandoffProof
			b"QmHandoffPhoto123".to_vec(),
			vec![seller, courier],
			2, // 2-of-2 quorum
		));

		let attestation_id = 0;

		// Verify not verified yet
		let attestation = BazariAttestation::attestations(attestation_id).unwrap();
		assert!(!attestation.verified);
		assert_eq!(attestation.signatures.len(), 0);

		// Seller co-signs (1/2)
		assert_ok!(BazariAttestation::co_sign(
			RuntimeOrigin::signed(seller),
			attestation_id,
		));

		let attestation = BazariAttestation::attestations(attestation_id).unwrap();
		assert!(!attestation.verified); // Still not verified (1/2)
		assert_eq!(attestation.signatures.len(), 1);
		assert!(attestation.signatures.contains(&seller));

		// Courier co-signs (2/2)
		assert_ok!(BazariAttestation::co_sign(
			RuntimeOrigin::signed(courier),
			attestation_id,
		));

		// Now verified ✅
		let attestation = BazariAttestation::attestations(attestation_id).unwrap();
		assert!(attestation.verified);
		assert_eq!(attestation.signatures.len(), 2);
		assert!(attestation.signatures.contains(&seller));
		assert!(attestation.signatures.contains(&courier));
		assert!(attestation.verified_at.is_some());

		// Event emitted
		System::assert_has_event(
			Event::ProofVerified {
				attestation_id,
				order_id,
			}
			.into(),
		);
	});
}

#[test]
fn delivery_proof_2_of_2_quorum_works() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);
		let buyer = account(3);

		// Create order
		let order_id = create_test_order(buyer, seller);

		// Courier submits DeliveryProof
		assert_ok!(BazariAttestation::submit_proof(
			RuntimeOrigin::signed(courier),
			order_id,
			1, // DeliveryProof
			b"QmDeliveryPhoto456".to_vec(),
			vec![courier, buyer],
			2, // 2-of-2 quorum
		));

		let attestation_id = 0;

		// Courier co-signs (1/2)
		assert_ok!(BazariAttestation::co_sign(
			RuntimeOrigin::signed(courier),
			attestation_id,
		));

		// Buyer co-signs (2/2)
		assert_ok!(BazariAttestation::co_sign(
			RuntimeOrigin::signed(buyer),
			attestation_id,
		));

		// Verified ✅
		let attestation = BazariAttestation::attestations(attestation_id).unwrap();
		assert!(attestation.verified);
		assert_eq!(attestation.proof_type, ProofType::DeliveryProof);
	});
}

#[test]
fn submit_proof_fails_order_not_found() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);

		// Try to submit proof for non-existent order
		assert_noop!(
			BazariAttestation::submit_proof(
				RuntimeOrigin::signed(seller),
				999, // Non-existent order
				0, // HandoffProof
				b"QmTest".to_vec(),
				vec![seller, courier],
				2,
			),
			Error::<Test>::OrderNotFound
		);
	});
}

#[test]
fn submit_proof_fails_unauthorized() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);
		let buyer = account(3);
		let unauthorized = account(4);

		// Create order
		let order_id = create_test_order(buyer, seller);

		// Unauthorized account tries to submit proof
		assert_noop!(
			BazariAttestation::submit_proof(
				RuntimeOrigin::signed(unauthorized),
				order_id,
				0, // HandoffProof
				b"QmTest".to_vec(),
				vec![seller, courier], // Unauthorized not in list
				2,
			),
			Error::<Test>::Unauthorized
		);
	});
}

#[test]
fn submit_proof_fails_invalid_quorum() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);
		let buyer = account(3);

		// Create order
		let order_id = create_test_order(buyer, seller);

		// Quorum = 0 (invalid)
		assert_noop!(
			BazariAttestation::submit_proof(
				RuntimeOrigin::signed(seller),
				order_id,
				0, // HandoffProof
				b"QmTest".to_vec(),
				vec![seller, courier],
				0, // Invalid: quorum = 0
			),
			Error::<Test>::InvalidQuorum
		);

		// Quorum > required_signers (invalid)
		assert_noop!(
			BazariAttestation::submit_proof(
				RuntimeOrigin::signed(seller),
				order_id,
				0, // HandoffProof
				b"QmTest".to_vec(),
				vec![seller, courier],
				3, // Invalid: quorum > 2 signers
			),
			Error::<Test>::InvalidQuorum
		);
	});
}

#[test]
fn co_sign_fails_already_signed() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);
		let buyer = account(3);

		// Create order
		let order_id = create_test_order(buyer, seller);

		// Submit proof
		assert_ok!(BazariAttestation::submit_proof(
			RuntimeOrigin::signed(seller),
			order_id,
			0, // HandoffProof
			b"QmTest".to_vec(),
			vec![seller, courier],
			2,
		));

		let attestation_id = 0;

		// Seller co-signs
		assert_ok!(BazariAttestation::co_sign(
			RuntimeOrigin::signed(seller),
			attestation_id,
		));

		// Try to sign again (double sign)
		assert_noop!(
			BazariAttestation::co_sign(RuntimeOrigin::signed(seller), attestation_id,),
			Error::<Test>::AlreadySigned
		);
	});
}

#[test]
fn co_sign_fails_unauthorized() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);
		let buyer = account(3);
		let unauthorized = account(4);

		// Create order
		let order_id = create_test_order(buyer, seller);

		// Submit proof
		assert_ok!(BazariAttestation::submit_proof(
			RuntimeOrigin::signed(seller),
			order_id,
			0, // HandoffProof
			b"QmTest".to_vec(),
			vec![seller, courier], // Only seller and courier
			2,
		));

		let attestation_id = 0;

		// Unauthorized tries to co-sign
		assert_noop!(
			BazariAttestation::co_sign(
				RuntimeOrigin::signed(unauthorized),
				attestation_id,
			),
			Error::<Test>::Unauthorized
		);
	});
}

#[test]
fn query_order_attestations_works() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);
		let buyer = account(3);

		// Create order
		let order_id = create_test_order(buyer, seller);

		// Submit HandoffProof
		assert_ok!(BazariAttestation::submit_proof(
			RuntimeOrigin::signed(seller),
			order_id,
			0, // HandoffProof
			b"QmHandoff".to_vec(),
			vec![seller, courier],
			2,
		));

		// Submit DeliveryProof
		assert_ok!(BazariAttestation::submit_proof(
			RuntimeOrigin::signed(courier),
			order_id,
			1, // DeliveryProof
			b"QmDelivery".to_vec(),
			vec![courier, buyer],
			2,
		));

		// Query all proofs for order
		let proofs = BazariAttestation::get_order_proofs(order_id);
		assert_eq!(proofs.len(), 2);

		// Verify HandoffProof exists
		assert!(proofs.iter().any(|(proof_type, _)| *proof_type == ProofType::HandoffProof));

		// Verify DeliveryProof exists
		assert!(proofs.iter().any(|(proof_type, _)| *proof_type == ProofType::DeliveryProof));
	});
}

#[test]
fn verify_proof_helper_works() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let courier = account(2);
		let buyer = account(3);

		// Create order
		let order_id = create_test_order(buyer, seller);

		// Submit proof
		assert_ok!(BazariAttestation::submit_proof(
			RuntimeOrigin::signed(seller),
			order_id,
			0, // HandoffProof
			b"QmTest".to_vec(),
			vec![seller, courier],
			2,
		));

		let attestation_id = 0;

		// Not verified yet
		assert!(!BazariAttestation::verify_proof(attestation_id));

		// Co-sign both
		assert_ok!(BazariAttestation::co_sign(
			RuntimeOrigin::signed(seller),
			attestation_id,
		));
		assert_ok!(BazariAttestation::co_sign(
			RuntimeOrigin::signed(courier),
			attestation_id,
		));

		// Now verified
		assert!(BazariAttestation::verify_proof(attestation_id));
	});
}
