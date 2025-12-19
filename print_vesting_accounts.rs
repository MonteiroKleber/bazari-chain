#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! sp-core = "41.0"
//! sp-runtime = "39.0"
//! hex = "0.4"
//! ```

use sp_core::blake2_256;
use sp_runtime::AccountId32;

fn account_from_seed(seed: &str) -> AccountId32 {
    AccountId32::from(blake2_256(seed.as_bytes()))
}

fn main() {
    println!("🔑 FASE 9 - Vesting Accounts");
    println!("============================\n");

    let founders = account_from_seed("bazari_vesting_founders");
    let team = account_from_seed("bazari_vesting_team");
    let partners = account_from_seed("bazari_vesting_partners");
    let marketing = account_from_seed("bazari_vesting_marketing");

    println!("Founders:  {}", founders);
    println!("Team:      {}", team);
    println!("Partners:  {}", partners);
    println!("Marketing: {}", marketing);

    println!("\nHex format:");
    println!("Founders:  0x{}", hex::encode(founders.as_ref()));
    println!("Team:      0x{}", hex::encode(team.as_ref()));
    println!("Partners:  0x{}", hex::encode(partners.as_ref()));
    println!("Marketing: 0x{}", hex::encode(marketing.as_ref()));
}
