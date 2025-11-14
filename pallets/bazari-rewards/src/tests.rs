use crate::{mock::*, Error, Event};
use frame_support::{assert_noop, assert_ok};

#[test]
fn mint_cashback_works() {
	new_test_ext().execute_with(|| {
		let buyer = 1;
		let order_amount = 1_000_000_000_000_000; // 1000 BZR

		// Mint cashback (should be 3% = 30 BZR, since 1000 >= 500)
		assert_ok!(BazariRewards::mint_cashback(
			RuntimeOrigin::root(),
			buyer,
			order_amount
		));

		// Verify ZARI balance (AssetId 1)
		assert_eq!(Assets::balance(1, buyer), 30_000_000_000_000); // 30 ZARI (3% of 1000)

		// Verify event
		System::assert_last_event(
			Event::CashbackMinted {
				user: buyer,
				amount: 30_000_000_000_000,
				order_amount,
			}
			.into(),
		);
	});
}

#[test]
fn mint_cashback_requires_root() {
	new_test_ext().execute_with(|| {
		let buyer = 1;
		let order_amount = 1_000_000_000_000_000;

		// User cannot mint cashback
		assert_noop!(
			BazariRewards::mint_cashback(RuntimeOrigin::signed(buyer), buyer, order_amount),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn cashback_rate_tiers_work() {
	new_test_ext().execute_with(|| {
		let buyer = 1;

		// Tier 1: 50 BZR → 1%
		let order_amount_1 = 50_000_000_000_000; // 50 BZR
		assert_ok!(BazariRewards::mint_cashback(
			RuntimeOrigin::root(),
			buyer,
			order_amount_1
		));
		assert_eq!(Assets::balance(1, buyer), 500_000_000_000); // 0.5 ZARI (1% of 50)

		// Tier 2: 300 BZR → 2%
		let order_amount_2 = 300_000_000_000_000; // 300 BZR
		assert_ok!(BazariRewards::mint_cashback(
			RuntimeOrigin::root(),
			buyer,
			order_amount_2
		));
		assert_eq!(Assets::balance(1, buyer), 6_500_000_000_000); // 0.5 + 6 = 6.5 ZARI

		// Tier 3: 600 BZR → 3%
		let order_amount_3 = 600_000_000_000_000; // 600 BZR
		assert_ok!(BazariRewards::mint_cashback(
			RuntimeOrigin::root(),
			buyer,
			order_amount_3
		));
		assert_eq!(Assets::balance(1, buyer), 24_500_000_000_000); // 6.5 + 18 = 24.5 ZARI
	});
}

#[test]
fn create_mission_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(BazariRewards::create_mission(
			RuntimeOrigin::root(),
			b"First Purchase".to_vec(),
			b"Complete your first order".to_vec(),
			0, // FirstPurchase
			0,
			100_000_000_000_000, // 100 ZARI
			1
		));

		let mission = BazariRewards::missions(0).unwrap();
		assert_eq!(mission.mission_id, 0);
		assert_eq!(mission.title.to_vec(), b"First Purchase".to_vec());
		assert_eq!(mission.reward_amount, 100_000_000_000_000);
		assert_eq!(mission.required_count, 1);
		assert_eq!(mission.is_active, true);

		// Verify event
		System::assert_last_event(Event::MissionCreated { mission_id: 0 }.into());
	});
}

#[test]
fn update_progress_works() {
	new_test_ext().execute_with(|| {
		let user = 1;

		// Create mission
		assert_ok!(BazariRewards::create_mission(
			RuntimeOrigin::root(),
			b"Complete 3 Orders".to_vec(),
			b"Buy 3 products".to_vec(),
			2, // CompleteNOrders
			3,
			50_000_000_000_000, // 50 ZARI
			3
		));

		// Update progress: increment by 1
		assert_ok!(BazariRewards::update_progress(
			RuntimeOrigin::root(),
			user,
			0,
			1
		));

		let progress = BazariRewards::user_progress(user, 0).unwrap();
		assert_eq!(progress.current_count, 1);
		assert_eq!(progress.is_completed, false);

		// Update progress: increment by 2 (total = 3, should complete)
		assert_ok!(BazariRewards::update_progress(
			RuntimeOrigin::root(),
			user,
			0,
			2
		));

		let progress = BazariRewards::user_progress(user, 0).unwrap();
		assert_eq!(progress.current_count, 3);
		assert_eq!(progress.is_completed, true);
		assert!(progress.completed_at.is_some());

		// Verify event
		System::assert_last_event(
			Event::MissionCompleted { user, mission_id: 0 }.into(),
		);
	});
}

#[test]
fn claim_reward_works() {
	new_test_ext().execute_with(|| {
		let user = 1;

		// Create mission
		assert_ok!(BazariRewards::create_mission(
			RuntimeOrigin::root(),
			b"First Purchase".to_vec(),
			b"Complete your first order".to_vec(),
			0, // FirstPurchase
			0,
			100_000_000_000_000, // 100 ZARI
			1
		));

		// Update progress to complete
		assert_ok!(BazariRewards::update_progress(
			RuntimeOrigin::root(),
			user,
			0,
			1
		));

		// Claim reward
		assert_ok!(BazariRewards::claim_reward(RuntimeOrigin::signed(user), 0));

		// Verify ZARI balance
		assert_eq!(Assets::balance(1, user), 100_000_000_000_000); // 100 ZARI

		// Verify progress updated
		let progress = BazariRewards::user_progress(user, 0).unwrap();
		assert_eq!(progress.is_claimed, true);

		// Verify event
		System::assert_last_event(
			Event::RewardClaimed {
				user,
				mission_id: 0,
				amount: 100_000_000_000_000,
			}
			.into(),
		);
	});
}

#[test]
fn claim_reward_fails_not_completed() {
	new_test_ext().execute_with(|| {
		let user = 1;

		// Create mission
		assert_ok!(BazariRewards::create_mission(
			RuntimeOrigin::root(),
			b"First Purchase".to_vec(),
			b"Complete your first order".to_vec(),
			0, // FirstPurchase
			0,
			100_000_000_000_000,
			1
		));

		// Try to claim without completing
		assert_noop!(
			BazariRewards::claim_reward(RuntimeOrigin::signed(user), 0),
			Error::<Test>::ProgressNotFound
		);

		// Update progress partially (not enough to complete)
		assert_ok!(BazariRewards::update_progress(
			RuntimeOrigin::root(),
			user,
			0,
			0 // Still at 0
		));

		// Try to claim (still not completed)
		assert_noop!(
			BazariRewards::claim_reward(RuntimeOrigin::signed(user), 0),
			Error::<Test>::MissionNotCompleted
		);
	});
}

#[test]
fn claim_reward_fails_already_claimed() {
	new_test_ext().execute_with(|| {
		let user = 1;

		// Create mission
		assert_ok!(BazariRewards::create_mission(
			RuntimeOrigin::root(),
			b"First Purchase".to_vec(),
			b"Complete your first order".to_vec(),
			0, // FirstPurchase
			0,
			100_000_000_000_000,
			1
		));

		// Complete mission
		assert_ok!(BazariRewards::update_progress(
			RuntimeOrigin::root(),
			user,
			0,
			1
		));

		// Claim reward
		assert_ok!(BazariRewards::claim_reward(RuntimeOrigin::signed(user), 0));

		// Try to claim again (double claim)
		assert_noop!(
			BazariRewards::claim_reward(RuntimeOrigin::signed(user), 0),
			Error::<Test>::AlreadyClaimed
		);
	});
}

#[test]
fn update_progress_fails_inactive_mission() {
	new_test_ext().execute_with(|| {
		let user = 1;

		// Create mission
		assert_ok!(BazariRewards::create_mission(
			RuntimeOrigin::root(),
			b"Test Mission".to_vec(),
			b"Test".to_vec(),
			0, // FirstPurchase
			0,
			50_000_000_000_000,
			1
		));

		// Deactivate mission manually (would normally be done via governance)
		crate::Missions::<Test>::mutate(0, |maybe_mission| {
			if let Some(ref mut mission) = maybe_mission {
				mission.is_active = false;
			}
		});

		// Try to update progress on inactive mission
		assert_noop!(
			BazariRewards::update_progress(RuntimeOrigin::root(), user, 0, 1),
			Error::<Test>::MissionInactive
		);
	});
}

#[test]
fn full_integration_test() {
	new_test_ext().execute_with(|| {
		let user = 1;
		let order_amount = 1_000_000_000_000_000; // 1000 BZR

		// Step 1: User makes purchase, backend mints cashback
		assert_ok!(BazariRewards::mint_cashback(
			RuntimeOrigin::root(),
			user,
			order_amount
		));
		assert_eq!(Assets::balance(1, user), 30_000_000_000_000); // 30 ZARI (3%, since 1000 >= 500)

		// Step 2: DAO creates mission
		assert_ok!(BazariRewards::create_mission(
			RuntimeOrigin::root(),
			b"First Purchase".to_vec(),
			b"Complete your first order".to_vec(),
			0, // FirstPurchase
			0,
			100_000_000_000_000, // 100 ZARI
			1
		));

		// Step 3: Backend updates user progress
		assert_ok!(BazariRewards::update_progress(
			RuntimeOrigin::root(),
			user,
			0,
			1
		));

		let progress = BazariRewards::user_progress(user, 0).unwrap();
		assert!(progress.is_completed);

		// Step 4: User claims reward
		assert_ok!(BazariRewards::claim_reward(RuntimeOrigin::signed(user), 0));

		// Verify total ZARI: 30 (cashback) + 100 (mission) = 130
		assert_eq!(Assets::balance(1, user), 130_000_000_000_000);

		let progress = BazariRewards::user_progress(user, 0).unwrap();
		assert!(progress.is_claimed);
	});
}
