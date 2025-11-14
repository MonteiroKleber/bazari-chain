use crate::{mock::*, Error, Event};
use frame_support::{assert_noop, assert_ok};

#[test]
fn set_platform_fee_works() {
	new_test_ext().execute_with(|| {
		// Initial fee is 5% (500 bps)
		assert_eq!(BazariFee::get_platform_fee_bps(), 500);

		// Update to 3% (300 bps)
		assert_ok!(BazariFee::set_platform_fee(RuntimeOrigin::root(), 300));

		assert_eq!(BazariFee::get_platform_fee_bps(), 300);

		// Check event
		System::assert_last_event(Event::PlatformFeeUpdated { new_fee_bps: 300 }.into());
	});
}

#[test]
fn set_platform_fee_fails_too_high() {
	new_test_ext().execute_with(|| {
		// Try to set 11% (1100 bps) - should fail
		assert_noop!(
			BazariFee::set_platform_fee(RuntimeOrigin::root(), 1100),
			Error::<Test>::FeeTooHigh
		);

		// 10% (1000 bps) should work
		assert_ok!(BazariFee::set_platform_fee(RuntimeOrigin::root(), 1000));
		assert_eq!(BazariFee::get_platform_fee_bps(), 1000);
	});
}

#[test]
fn set_platform_fee_fails_not_root() {
	new_test_ext().execute_with(|| {
		// Try to set fee as non-root - should fail
		assert_noop!(
			BazariFee::set_platform_fee(RuntimeOrigin::signed(1), 300),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn calculate_split_no_affiliate() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let buyer = account(2);
		let amount = 10_000u128;
		let order_id = 1u64;

		// Calculate split (no affiliate)
		let splits = BazariFee::calculate_split(order_id, seller, buyer, amount).unwrap();

		// Should have 2 splits: platform + seller
		assert_eq!(splits.len(), 2);

		// Platform fee: 5% of 10,000 = 500
		let (platform_account, platform_amount, _) = &splits[0];
		assert_eq!(*platform_account, account(999)); // Treasury
		assert_eq!(*platform_amount, 500);

		// Seller: 95% of 10,000 = 9,500
		let (seller_account, seller_amount, _) = &splits[1];
		assert_eq!(*seller_account, seller);
		assert_eq!(*seller_amount, 9_500);

		// Verify sum equals total
		let total: u128 = splits.iter().map(|(_, amt, _)| *amt).sum();
		assert_eq!(total, amount);
	});
}

#[test]
fn calculate_split_with_affiliate() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let buyer = account(3);
		let referrer = account(2);
		let amount = 10_000u128;
		let order_id = 1u64;

		// Register affiliate
		assert_ok!(BazariAffiliate::register_referral(RuntimeOrigin::signed(buyer), referrer));

		// Calculate split (with affiliate)
		let splits = BazariFee::calculate_split(order_id, seller, buyer, amount).unwrap();

		// Should have 3 splits: platform + affiliate + seller
		assert_eq!(splits.len(), 3);

		// Platform fee: 5% of 10,000 = 500
		let (platform_account, platform_amount, _) = &splits[0];
		assert_eq!(*platform_account, account(999)); // Treasury
		assert_eq!(*platform_amount, 500);

		// Affiliate commission: 5% of 10,000 = 500 (L0 rate)
		let (affiliate_account, affiliate_amount, _) = &splits[1];
		assert_eq!(*affiliate_account, referrer);
		assert_eq!(*affiliate_amount, 500);

		// Seller: remainder = 10,000 - 500 - 500 = 9,000
		let (seller_account, seller_amount, _) = &splits[2];
		assert_eq!(*seller_account, seller);
		assert_eq!(*seller_amount, 9_000);

		// Verify sum equals total
		let total: u128 = splits.iter().map(|(_, amt, _)| *amt).sum();
		assert_eq!(total, amount);
	});
}

#[test]
fn calculate_split_sum_equals_total() {
	new_test_ext().execute_with(|| {
		// Test various amounts to ensure no rounding errors
		let test_amounts = vec![1_000u128, 5_000, 10_000, 99_999, 123_456];

		for amount in test_amounts {
			let seller = account(1);
			let buyer = account(2);
			let order_id = 1u64;

			let splits = BazariFee::calculate_split(order_id, seller, buyer, amount).unwrap();

			// Verify sum equals total
			let total: u128 = splits.iter().map(|(_, amt, _)| *amt).sum();
			assert_eq!(total, amount, "Sum mismatch for amount {}", amount);
		}
	});
}

#[test]
fn calculate_split_fails_below_minimum() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let buyer = account(2);
		let amount = 50u128; // Below MinOrderAmount (100)
		let order_id = 1u64;

		assert_noop!(
			BazariFee::calculate_split(order_id, seller, buyer, amount),
			Error::<Test>::InvalidAmount
		);
	});
}

#[test]
fn get_treasury_account_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(BazariFee::get_treasury_account(), account(999));
	});
}

#[test]
fn fee_config_defaults() {
	new_test_ext().execute_with(|| {
		let config = BazariFee::fee_config();

		assert_eq!(config.platform_fee_bps, 500); // 5%
		assert_eq!(config.treasury_account, account(999));
		assert_eq!(config.min_order_amount, 100);
	});
}

#[test]
fn split_event_emitted() {
	new_test_ext().execute_with(|| {
		let seller = account(1);
		let buyer = account(2);
		let amount = 10_000u128;
		let order_id = 1u64;

		let _ = BazariFee::calculate_split(order_id, seller, buyer, amount).unwrap();

		// Check event was emitted
		System::assert_has_event(
			Event::SplitCalculated {
				order_id,
				total_amount: amount,
				platform_fee: 500,
				affiliate_commission: 0,
				seller_amount: 9_500,
			}.into()
		);
	});
}
