use crate::{mock::*, Error, Event};
use frame_support::{assert_noop, assert_ok};

// Helper function to register identity
fn register_identity(account: AccountId) {
	let handle = format!("handle{}", account);
	assert_ok!(BazariIdentity::mint_profile(
		RuntimeOrigin::signed(1), // Anyone can mint for others
		account,
		handle.as_bytes().to_vec(),
		b"QmTest".to_vec()
	));
}

#[test]
fn register_courier_works() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		// Register identity first
		register_identity(courier);

		// Get initial balance
		let initial_balance = Balances::free_balance(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas.clone()
		));

		// Check event
		System::assert_last_event(
			Event::CourierRegistered { account: courier, stake }.into(),
		);

		// Check storage
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.stake, stake);
		assert_eq!(courier_data.reputation_score, 500);
		assert_eq!(courier_data.service_areas.to_vec(), service_areas);
		assert_eq!(courier_data.is_active, true);
		assert_eq!(courier_data.total_deliveries, 0);
		assert_eq!(courier_data.successful_deliveries, 0);

		// Check stake is reserved
		assert_eq!(Balances::reserved_balance(courier), stake);
		assert_eq!(Balances::free_balance(courier), initial_balance - stake);
	});
}

#[test]
fn register_courier_fails_without_identity() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		// Try to register without identity
		assert_noop!(
			BazariFulfillment::register_courier(
				RuntimeOrigin::signed(courier),
				stake,
				service_areas
			),
			Error::<Test>::IdentityRequired
		);
	});
}

#[test]
fn register_courier_fails_insufficient_stake() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 500; // Less than MinCourierStake (1000)
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		assert_noop!(
			BazariFulfillment::register_courier(
				RuntimeOrigin::signed(courier),
				stake,
				service_areas
			),
			Error::<Test>::InsufficientStake
		);
	});
}

#[test]
fn register_courier_fails_already_registered() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register first time
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas.clone()
		));

		// Try to register again
		assert_noop!(
			BazariFulfillment::register_courier(
				RuntimeOrigin::signed(courier),
				stake,
				service_areas
			),
			Error::<Test>::CourierAlreadyRegistered
		);
	});
}

#[test]
fn register_courier_fails_too_many_service_areas() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 1000;
		// More than MaxServiceAreas (10)
		let service_areas = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

		register_identity(courier);

		assert_noop!(
			BazariFulfillment::register_courier(
				RuntimeOrigin::signed(courier),
				stake,
				service_areas
			),
			Error::<Test>::TooManyServiceAreas
		);
	});
}

#[test]
fn assign_courier_works() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let seller = account(3);
		let order_id = 1;
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Assign courier to order
		assert_ok!(BazariFulfillment::assign_courier(
			RuntimeOrigin::signed(seller),
			order_id,
			courier
		));

		// Check event
		System::assert_last_event(Event::CourierAssigned { order_id, courier }.into());

		// Check storage
		assert_eq!(BazariFulfillment::order_couriers(order_id), Some(courier));
		assert_eq!(BazariFulfillment::courier_deliveries(courier).to_vec(), vec![order_id]);
	});
}

#[test]
fn assign_courier_fails_not_registered() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let seller = account(3);
		let order_id = 1;

		assert_noop!(
			BazariFulfillment::assign_courier(RuntimeOrigin::signed(seller), order_id, courier),
			Error::<Test>::CourierNotFound
		);
	});
}

#[test]
fn assign_courier_fails_inactive() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let seller = account(3);
		let order_id = 1;
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Deactivate courier
		assert_ok!(BazariFulfillment::deactivate_courier(RuntimeOrigin::signed(courier)));

		// Try to assign inactive courier
		assert_noop!(
			BazariFulfillment::assign_courier(RuntimeOrigin::signed(seller), order_id, courier),
			Error::<Test>::CourierInactive
		);
	});
}

#[test]
fn complete_delivery_works() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let seller = account(3);
		let order_id = 1;
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Assign courier to order
		assert_ok!(BazariFulfillment::assign_courier(
			RuntimeOrigin::signed(seller),
			order_id,
			courier
		));

		// Complete delivery
		assert_ok!(BazariFulfillment::complete_delivery(
			RuntimeOrigin::signed(courier),
			order_id
		));

		// Check event
		System::assert_last_event(Event::DeliveryCompleted { order_id, courier }.into());

		// Check courier stats updated
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.total_deliveries, 1);
		assert_eq!(courier_data.successful_deliveries, 1);
		assert_eq!(courier_data.reputation_score, 510); // 500 + 10
	});
}

#[test]
fn complete_delivery_increases_reputation() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let seller = account(3);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Complete 10 deliveries
		for i in 1..=10 {
			assert_ok!(BazariFulfillment::assign_courier(
				RuntimeOrigin::signed(seller),
				i,
				courier
			));
			assert_ok!(BazariFulfillment::complete_delivery(
				RuntimeOrigin::signed(courier),
				i
			));
		}

		// Check reputation increased
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.reputation_score, 600); // 500 + (10 * 10)
		assert_eq!(courier_data.total_deliveries, 10);
		assert_eq!(courier_data.successful_deliveries, 10);
	});
}

#[test]
fn complete_delivery_fails_wrong_courier() {
	new_test_ext().execute_with(|| {
		let courier1 = account(1);
		let courier2 = account(2);
		let seller = account(3);
		let order_id = 1;
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier1);
		register_identity(courier2);

		// Register both couriers
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier1),
			stake,
			service_areas.clone()
		));
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier2),
			stake,
			service_areas
		));

		// Assign courier1 to order
		assert_ok!(BazariFulfillment::assign_courier(
			RuntimeOrigin::signed(seller),
			order_id,
			courier1
		));

		// Try to complete with courier2
		assert_noop!(
			BazariFulfillment::complete_delivery(RuntimeOrigin::signed(courier2), order_id),
			Error::<Test>::Unauthorized
		);
	});
}

#[test]
fn complete_delivery_fails_order_not_assigned() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let order_id = 1;
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Try to complete unassigned order
		assert_noop!(
			BazariFulfillment::complete_delivery(RuntimeOrigin::signed(courier), order_id),
			Error::<Test>::OrderNotAssigned
		);
	});
}

#[test]
fn slash_courier_works() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let seller = account(3);
		let order_id = 1;
		let stake = 2000; // Higher stake so it stays above minimum after slash
		let service_areas = vec![1, 2, 3];
		let slash_amount = 200;

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Assign and complete delivery
		assert_ok!(BazariFulfillment::assign_courier(
			RuntimeOrigin::signed(seller),
			order_id,
			courier
		));

		// Slash courier (using Root as DAO)
		assert_ok!(BazariFulfillment::slash_courier(
			RuntimeOrigin::root(),
			courier,
			slash_amount,
			b"Bad delivery".to_vec()
		));

		// Check courier data
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.stake, stake - slash_amount); // 1800
		assert_eq!(courier_data.reputation_score, 400); // 500 - 100
		assert_eq!(courier_data.disputed_deliveries, 1);
		assert_eq!(courier_data.is_active, true); // Still above minimum (1800 >= 1000)
	});
}

#[test]
fn slash_courier_deactivates_if_stake_too_low() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];
		let slash_amount = 500; // Slash to below minimum (1000)

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Slash courier below minimum stake
		assert_ok!(BazariFulfillment::slash_courier(
			RuntimeOrigin::root(),
			courier,
			slash_amount,
			b"Bad delivery".to_vec()
		));

		// Check courier deactivated
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.is_active, false);
	});
}

#[test]
fn slash_courier_fails_not_dao() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let attacker = account(2);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];
		let slash_amount = 200;

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Try to slash without DAO origin
		assert_noop!(
			BazariFulfillment::slash_courier(
				RuntimeOrigin::signed(attacker),
				courier,
				slash_amount,
				b"Bad delivery".to_vec()
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn update_merkle_root_works() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];
		let merkle_root = [1u8; 32];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Update Merkle root (using Root)
		assert_ok!(BazariFulfillment::update_reviews_merkle_root(
			RuntimeOrigin::root(),
			courier,
			merkle_root
		));

		// Check event
		System::assert_last_event(
			Event::ReviewsMerkleRootUpdated { courier, merkle_root }.into(),
		);

		// Check storage updated
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.reviews_merkle_root, merkle_root);
	});
}

#[test]
fn update_merkle_root_fails_not_root() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let attacker = account(2);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];
		let merkle_root = [1u8; 32];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Try to update Merkle root without Root origin
		assert_noop!(
			BazariFulfillment::update_reviews_merkle_root(
				RuntimeOrigin::signed(attacker),
				courier,
				merkle_root
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn deactivate_courier_works() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Deactivate
		assert_ok!(BazariFulfillment::deactivate_courier(RuntimeOrigin::signed(courier)));

		// Check event
		System::assert_last_event(Event::CourierDeactivated { account: courier }.into());

		// Check storage
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.is_active, false);
	});
}

#[test]
fn reactivate_courier_works() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Deactivate
		assert_ok!(BazariFulfillment::deactivate_courier(RuntimeOrigin::signed(courier)));

		// Reactivate
		assert_ok!(BazariFulfillment::reactivate_courier(RuntimeOrigin::signed(courier)));

		// Check event
		System::assert_last_event(Event::CourierReactivated { account: courier }.into());

		// Check storage
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.is_active, true);
	});
}

#[test]
fn reactivate_courier_fails_insufficient_stake() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Slash below minimum
		assert_ok!(BazariFulfillment::slash_courier(
			RuntimeOrigin::root(),
			courier,
			500,
			b"Bad".to_vec()
		));

		// Try to reactivate with insufficient stake
		assert_noop!(
			BazariFulfillment::reactivate_courier(RuntimeOrigin::signed(courier)),
			Error::<Test>::InsufficientStake
		);
	});
}

#[test]
fn increase_stake_works() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let initial_stake = 1000;
		let additional_stake = 500;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			initial_stake,
			service_areas
		));

		// Increase stake
		assert_ok!(BazariFulfillment::increase_stake(
			RuntimeOrigin::signed(courier),
			additional_stake
		));

		// Check event
		System::assert_last_event(
			Event::CourierStakeIncreased {
				account: courier,
				new_stake: initial_stake + additional_stake,
			}
			.into(),
		);

		// Check storage
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.stake, initial_stake + additional_stake);

		// Check balance
		assert_eq!(
			Balances::reserved_balance(courier),
			initial_stake + additional_stake
		);
	});
}

#[test]
fn reputation_caps_at_1000() {
	new_test_ext().execute_with(|| {
		let courier = account(1);
		let seller = account(3);
		let stake = 1000;
		let service_areas = vec![1, 2, 3];

		register_identity(courier);

		// Register courier
		assert_ok!(BazariFulfillment::register_courier(
			RuntimeOrigin::signed(courier),
			stake,
			service_areas
		));

		// Complete 100 deliveries (should cap reputation at 1000)
		for i in 1..=100 {
			assert_ok!(BazariFulfillment::assign_courier(
				RuntimeOrigin::signed(seller),
				i,
				courier
			));
			assert_ok!(BazariFulfillment::complete_delivery(
				RuntimeOrigin::signed(courier),
				i
			));
		}

		// Check reputation capped at 1000
		let courier_data = BazariFulfillment::couriers(courier).unwrap();
		assert_eq!(courier_data.reputation_score, 1000);
	});
}
