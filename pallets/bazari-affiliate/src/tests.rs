use crate::{mock::*, Error, Event};
use frame_support::{assert_noop, assert_ok, traits::Currency};

#[test]
fn register_referral_works() {
	new_test_ext().execute_with(|| {
		let referrer = account(1);
		let referee = account(2);

		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(referee), referrer));

		System::assert_last_event(Event::ReferralRegistered { referrer, referee }.into());

		assert_eq!(BazariAffiliate::referrer_of(referee), Some(referrer));
		assert_eq!(BazariAffiliate::direct_referrals(referrer).to_vec(), vec![referee]);

		let stats = BazariAffiliate::affiliate_stats(referrer).unwrap();
		assert_eq!(stats.direct_referrals, 1);
		assert_eq!(stats.total_referrals, 1);
	});
}

#[test]
fn register_referral_fails_self_referral() {
	new_test_ext().execute_with(|| {
		let account = account(1);

		assert_noop!(
			BazariAffiliate::register_referral(RuntimeOrigin::signed(account), account),
			Error::<Test>::SelfReferral
		);
	});
}

#[test]
fn register_referral_fails_already_referred() {
	new_test_ext().execute_with(|| {
		let referrer1 = account(1);
		let referrer2 = account(2);
		let referee = account(3);

		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(referee), referrer1));

		assert_noop!(
			BazariAffiliate::register_referral(RuntimeOrigin::signed(referee), referrer2),
			Error::<Test>::AlreadyReferred
		);
	});
}

#[test]
fn register_referral_fails_circular_reference() {
	new_test_ext().execute_with(|| {
		let acc1 = account(1);
		let acc2 = account(2);

		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(acc2), acc1));

		assert_noop!(
			BazariAffiliate::register_referral(RuntimeOrigin::signed(acc1), acc2),
			Error::<Test>::CircularReference
		);
	});
}

#[test]
fn get_referral_path_works() {
	new_test_ext().execute_with(|| {
		// Create chain: 1 <- 2 <- 3 <- 4
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(2), 1));
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(3), 2));
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(4), 3));

		let path = BazariAffiliate::get_referral_path(&4);
		assert_eq!(path, vec![3, 2, 1]);

		let path2 = BazariAffiliate::get_referral_path(&2);
		assert_eq!(path2, vec![1]);
	});
}

#[test]
fn distribute_commissions_works() {
	new_test_ext().execute_with(|| {
		// Create referral chain: 1 <- 2 <- 3 (buyer)
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(2), 1));
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(3), 2));

		// Create order
		let order_id = 0; // First order gets ID 0
		let buyer = account(3);
		let seller = account(6);

		assert_ok!(BazariCommerce::create_order(
			RuntimeOrigin::signed(buyer),
			0, // source: Marketplace
			None, // thread_id
			seller,
			None, // store_id
			vec![(None, b"Item".to_vec(), 1, 1000)] // (product_id, name, quantity, price)
		));

		let order_amount = 1000;
		let initial_balance_1 = Balances::free_balance(1);
		let initial_balance_2 = Balances::free_balance(2);

		// Distribute commissions
		assert_ok!(BazariAffiliate::distribute_commissions(
			RuntimeOrigin::root(),
			order_id,
			buyer,
			order_amount
		));

		// L0: account(2) gets 5% = 50
		// L1: account(1) gets 2.5% = 25
		assert_eq!(Balances::free_balance(2), initial_balance_2 + 50);
		assert_eq!(Balances::free_balance(1), initial_balance_1 + 25);

		// Check stats
		let stats1 = BazariAffiliate::affiliate_stats(1).unwrap();
		assert_eq!(stats1.total_commission_earned, 25);

		let stats2 = BazariAffiliate::affiliate_stats(2).unwrap();
		assert_eq!(stats2.total_commission_earned, 50);

		// Check commission history
		let history = BazariAffiliate::order_commissions(order_id);
		assert_eq!(history.len(), 2);
		assert_eq!(history[0], (2, 50, 0)); // L0
		assert_eq!(history[1], (1, 25, 1)); // L1
	});
}

#[test]
fn distribute_commissions_5_levels() {
	new_test_ext().execute_with(|| {
		// Create 5-level chain: 1 <- 2 <- 3 <- 4 <- 5 <- 6 (buyer)
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(2), 1));
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(3), 2));
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(4), 3));
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(5), 4));
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(6), 5));

		// Create order
		let order_id = 0; // First order gets ID 0
		let buyer = account(6);
		let seller = account(1); // Use account 1 as seller since it has balance

		// First give account 1 enough balance to be seller
		let _ = Balances::deposit_into_existing(&seller, 10_000);

		assert_ok!(BazariCommerce::create_order(
			RuntimeOrigin::signed(buyer),
			0, // source: Marketplace
			None, // thread_id
			seller,
			None, // store_id
			vec![(None, b"Item".to_vec(), 1, 10000)] // (product_id, name, quantity, price)
		));

		let order_amount = 10000;

		// Distribute commissions
		assert_ok!(BazariAffiliate::distribute_commissions(
			RuntimeOrigin::root(),
			order_id,
			buyer,
			order_amount
		));

		// Check all 5 levels received commissions
		// L0: 5 gets 5% = 500
		// L1: 4 gets 2.5% = 250
		// L2: 3 gets 1.25% = 125
		// L3: 2 gets 0.62% = 62
		// L4: 1 gets 0.31% = 31

		let stats5 = BazariAffiliate::affiliate_stats(5).unwrap();
		assert_eq!(stats5.total_commission_earned, 500);

		let stats4 = BazariAffiliate::affiliate_stats(4).unwrap();
		assert_eq!(stats4.total_commission_earned, 250);

		let stats3 = BazariAffiliate::affiliate_stats(3).unwrap();
		assert_eq!(stats3.total_commission_earned, 125);

		let stats2 = BazariAffiliate::affiliate_stats(2).unwrap();
		assert_eq!(stats2.total_commission_earned, 62);

		let stats1 = BazariAffiliate::affiliate_stats(1).unwrap();
		assert_eq!(stats1.total_commission_earned, 31);
	});
}

#[test]
fn update_merkle_root_works() {
	new_test_ext().execute_with(|| {
		let account = account(1);
		let merkle_root = [1u8; 32];

		assert_ok!(BazariAffiliate::update_merkle_root(
			RuntimeOrigin::root(),
			account,
			merkle_root
		));

		System::assert_last_event(Event::MerkleRootUpdated { account, root: merkle_root }.into());

		let stats = BazariAffiliate::affiliate_stats(account).unwrap();
		assert_eq!(stats.merkle_root, merkle_root);
	});
}

#[test]
fn update_merkle_root_fails_not_root() {
	new_test_ext().execute_with(|| {
		let account = account(1);
		let merkle_root = [1u8; 32];

		assert_noop!(
			BazariAffiliate::update_merkle_root(RuntimeOrigin::signed(account), account, merkle_root),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn distribute_commissions_fails_order_not_found() {
	new_test_ext().execute_with(|| {
		let order_id = 999;
		let buyer = account(1);
		let order_amount = 1000;

		assert_noop!(
			BazariAffiliate::distribute_commissions(RuntimeOrigin::root(), order_id, buyer, order_amount),
			Error::<Test>::OrderNotFound
		);
	});
}

#[test]
fn total_referrals_count_correctly() {
	new_test_ext().execute_with(|| {
		// 1 <- 2 <- 3
		//   <- 4
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(2), 1));
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(3), 2));
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(4), 1));

		let stats1 = BazariAffiliate::affiliate_stats(1).unwrap();
		assert_eq!(stats1.direct_referrals, 2); // 2 and 4
		assert_eq!(stats1.total_referrals, 3); // 2, 3, and 4

		let stats2 = BazariAffiliate::affiliate_stats(2).unwrap();
		assert_eq!(stats2.direct_referrals, 1); // only 3
		assert_eq!(stats2.total_referrals, 1); // only 3
	});
}
