// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{AccountId, BalancesConfig, RuntimeGenesisConfig, SudoConfig, Balance, BZR};
use alloc::{vec, vec::Vec};
use frame_support::build_struct_json_patch;
use serde_json::Value;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_genesis_builder::{self, PresetId};
use sp_keyring::Sr25519Keyring;

// ===== FASE 9: VESTING ACCOUNTS =====
// Contas dedicadas para cada categoria de vesting
// Em produção, estas seriam multisigs ou contas controladas por governance

/// Gera AccountId determinístico a partir de seed
fn account_from_seed(seed: &str) -> AccountId {
    sp_runtime::AccountId32::from(sp_core::blake2_256(seed.as_bytes()))
}

/// Founders account (150M BZR, 4 years, 1 year cliff)
fn founders_account() -> AccountId {
    account_from_seed("bazari_vesting_founders")
}

/// Team account (100M BZR, 3 years, 6 months cliff)
fn team_account() -> AccountId {
    account_from_seed("bazari_vesting_team")
}

/// Partners account (80M BZR, 2 years, 3 months cliff)
fn partners_account() -> AccountId {
    account_from_seed("bazari_vesting_partners")
}

/// Marketing account (50M BZR, 1 year, no cliff)
fn marketing_account() -> AccountId {
    account_from_seed("bazari_vesting_marketing")
}

// Vesting schedules calculation (block time = 6 seconds)
// 1 minute = 10 blocks
// 1 hour = 600 blocks
// 1 day = 14,400 blocks
// 1 month (30 days) = 432,000 blocks
// 1 year (365 days) = 5,256,000 blocks

/// Cria vesting schedule para Founders
/// 150M BZR, 4 anos (21,024,000 blocks), 1 ano cliff (5,256,000 blocks)
/// Retorna: (balance, begin_block, length_blocks, liquid)
fn founders_vesting_schedule() -> (Balance, u32, u32, Balance) {
    let balance = 150_000_000 * BZR;
    let begin = 5_256_000u32;        // 1 year cliff
    let length = 21_024_000u32;      // 4 years total vesting
    let liquid = 0u128;              // Nothing liquid
    (balance, begin, length, liquid)
}

/// Cria vesting schedule para Team
/// 100M BZR, 3 anos (15,768,000 blocks), 6 meses cliff (2,628,000 blocks)
/// Retorna: (balance, begin_block, length_blocks, liquid)
fn team_vesting_schedule() -> (Balance, u32, u32, Balance) {
    let balance = 100_000_000 * BZR;
    let begin = 2_628_000u32;        // 6 months cliff
    let length = 15_768_000u32;      // 3 years total vesting
    let liquid = 0u128;
    (balance, begin, length, liquid)
}

/// Cria vesting schedule para Partners
/// 80M BZR, 2 anos (10,512,000 blocks), 3 meses cliff (1,314,000 blocks)
/// Retorna: (balance, begin_block, length_blocks, liquid)
fn partners_vesting_schedule() -> (Balance, u32, u32, Balance) {
    let balance = 80_000_000 * BZR;
    let begin = 1_314_000u32;        // 3 months cliff
    let length = 10_512_000u32;      // 2 years total vesting
    let liquid = 0u128;
    (balance, begin, length, liquid)
}

/// Cria vesting schedule para Marketing
/// 50M BZR, 1 ano (5,256,000 blocks), sem cliff
/// Retorna: (balance, begin_block, length_blocks, liquid)
fn marketing_vesting_schedule() -> (Balance, u32, u32, Balance) {
    let balance = 50_000_000 * BZR;
    let begin = 0u32;                // no cliff
    let length = 5_256_000u32;       // 1 year total vesting
    let liquid = 0u128;
    (balance, begin, length, liquid)
}

// Returns the genesis config presets populated with given parameters.
fn testnet_genesis(
    initial_authorities: Vec<(AuraId, GrandpaId)>,
    endowed_accounts: Vec<AccountId>,
    root: AccountId,
) -> Value {
    // Preparar contas com BZR inicial
    let mut bzr_balances = endowed_accounts
        .iter()
        .cloned()
        .map(|k| (k, 1u128 << 60)) // ~1.15M BZR per account
        .collect::<Vec<_>>();

    // ===== FASE 9: VESTING BALANCES =====
    // Adicionar balances para contas de vesting
    // Estas contas terão BZR locked com schedules de vesting
    let (founders_balance, _, _, _) = founders_vesting_schedule();
    let (team_balance, _, _, _) = team_vesting_schedule();
    let (partners_balance, _, _, _) = partners_vesting_schedule();
    let (marketing_balance, _, _, _) = marketing_vesting_schedule();

    bzr_balances.extend(vec![
        (founders_account(), founders_balance),
        (team_account(), team_balance),
        (partners_account(), partners_balance),
        (marketing_account(), marketing_balance),
    ]);

    // ZARI: 21 milhões com 12 decimais = 21_000_000 * 10^12
    let zari_total_supply: u128 = 21_000_000 * 1_000_000_000_000u128;

    // Owner do ZARI (Alice em dev, multisig em produção)
    let zari_owner = root.clone();

    build_struct_json_patch!(RuntimeGenesisConfig {
        balances: BalancesConfig {
            balances: bzr_balances,
        },
        aura: pallet_aura::GenesisConfig {
            authorities: initial_authorities
                .iter()
                .map(|x| (x.0.clone()))
                .collect::<Vec<_>>(),
        },
        grandpa: pallet_grandpa::GenesisConfig {
            authorities: initial_authorities
                .iter()
                .map(|x| (x.1.clone(), 1))
                .collect::<Vec<_>>(),
        },
        sudo: SudoConfig { key: Some(root) },

        // ===== FASE 3: ZARI GENESIS =====
        assets: pallet_assets::GenesisConfig {
            // Criar asset ZARI (ID=1)
            assets: vec![
                // (asset_id, owner, is_sufficient, min_balance)
                (1, zari_owner.clone(), true, 1u128), // min_balance = 1 planck
            ],
            // Metadata do ZARI
            metadata: vec![
                // (asset_id, name, symbol, decimals)
                (1, b"Bazari Governance Token".to_vec(), b"ZARI".to_vec(), 12),
            ],
            // Alocar supply total para owner
            accounts: vec![
                // (asset_id, account, balance)
                (1, zari_owner, zari_total_supply),
            ],
        },

        // ===== FASE 9: VESTING GENESIS =====
        vesting: pallet_vesting::GenesisConfig {
            vesting: vec![
                // Founders: 150M BZR, 4 anos, 1 ano cliff
                {
                    let (_, begin, length, liquid) = founders_vesting_schedule();
                    (founders_account(), begin, length, liquid)
                },
                // Team: 100M BZR, 3 anos, 6 meses cliff
                {
                    let (_, begin, length, liquid) = team_vesting_schedule();
                    (team_account(), begin, length, liquid)
                },
                // Partners: 80M BZR, 2 anos, 3 meses cliff
                {
                    let (_, begin, length, liquid) = partners_vesting_schedule();
                    (partners_account(), begin, length, liquid)
                },
                // Marketing: 50M BZR, 1 ano, sem cliff
                {
                    let (_, begin, length, liquid) = marketing_vesting_schedule();
                    (marketing_account(), begin, length, liquid)
                },
            ],
        },
    })
}

/// Return the development genesis config.
pub fn development_config_genesis() -> Value {
    testnet_genesis(
        vec![(
            sp_keyring::Sr25519Keyring::Alice.public().into(),
            sp_keyring::Ed25519Keyring::Alice.public().into(),
        )],
        vec![
            Sr25519Keyring::Alice.to_account_id(),
            Sr25519Keyring::Bob.to_account_id(),
            Sr25519Keyring::AliceStash.to_account_id(),
            Sr25519Keyring::BobStash.to_account_id(),
        ],
        sp_keyring::Sr25519Keyring::Alice.to_account_id(),
    )
}

/// Return the local genesis config preset.
pub fn local_config_genesis() -> Value {
    testnet_genesis(
        vec![
            (
                sp_keyring::Sr25519Keyring::Alice.public().into(),
                sp_keyring::Ed25519Keyring::Alice.public().into(),
            ),
            (
                sp_keyring::Sr25519Keyring::Bob.public().into(),
                sp_keyring::Ed25519Keyring::Bob.public().into(),
            ),
        ],
        Sr25519Keyring::iter()
            .filter(|v| v != &Sr25519Keyring::One && v != &Sr25519Keyring::Two)
            .map(|v| v.to_account_id())
            .collect::<Vec<_>>(),
        Sr25519Keyring::Alice.to_account_id(),
    )
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
        _ => return None,
    };
    Some(
        serde_json::to_string(&patch)
            .expect("serialization to json is expected to work. qed.")
            .into_bytes(),
    )
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
    vec![
        PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
        PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
    ]
}
