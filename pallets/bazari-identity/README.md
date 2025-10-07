# Pallet Bazari Identity

**Soulbound NFT Profile System with Reputation and Badges**

A Substrate pallet that implements non-transferable (soulbound) NFT-based user profiles with on-chain reputation tracking and verifiable badge system for the Bazari marketplace.

## Overview

This pallet provides:

- **Soulbound NFT Profiles**: Each blockchain account can mint exactly one non-transferable profile NFT
- **Reputation System**: On-chain reputation score with tier system (Bronze, Prata, Ouro, Diamante)
- **Badge System**: Verifiable badges awarded by authorized modules with revocation support
- **Handle Management**: Unique usernames with 30-day cooldown between changes
- **IPFS Metadata**: Off-chain metadata storage with CID references
- **Penalty Tracking**: Record violations and penalties with expiration
- **Module Authorization**: Fine-grained permission system for reputation and badge management

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Pallet Bazari Identity                    │
├─────────────────────────────────────────────────────────────┤
│  Storage:                                                    │
│  • NextProfileId (counter)                                   │
│  • OwnerProfile (AccountId → ProfileId)                      │
│  • ProfileOwner (ProfileId → AccountId)                      │
│  • HandleToProfile (Handle → ProfileId)                      │
│  • MetadataCid (ProfileId → IPFS CID)                        │
│  • Reputation (ProfileId → i32)                              │
│  • Badges (ProfileId → BoundedBTreeSet<Badge>)               │
│  • Penalties (ProfileId → BoundedVec<Penalty>)               │
│  • HandleHistory (ProfileId → BoundedVec<HandleRecord>)      │
│  • AuthorizedModules (BoundedBTreeSet<ModuleId>)             │
│  • AuthorizedIssuers (BoundedBTreeSet<ModuleId>)             │
│  • PenaltyRevokers (BoundedBTreeSet<ModuleId>)               │
│  • Paused (bool)                                             │
├─────────────────────────────────────────────────────────────┤
│  Extrinsics:                                                 │
│  • mint_profile (origin, owner, handle, cid)                 │
│  • update_metadata_cid (origin, profile_id, cid)             │
│  • set_handle (origin, profile_id, new_handle)               │
│  • increment_reputation (origin, profile_id, points, reason) │
│  • decrement_reputation (origin, profile_id, points, reason) │
│  • award_badge (origin, profile_id, code, issuer)            │
│  • revoke_badge (origin, profile_id, code)                   │
│  • add_penalty (origin, profile_id, reason, severity, ...)   │
│  • revoke_penalty (origin, profile_id, penalty_id)           │
│  • authorize_module (root, module_id)                        │
│  • authorize_issuer (root, module_id)                        │
│  • set_paused (root, paused)                                 │
└─────────────────────────────────────────────────────────────┘
```

## Storage Items

### Core Profile Storage

#### `NextProfileId`
```rust
pub type NextProfileId<T> = StorageValue<_, ProfileId, ValueQuery>;
```
Auto-incrementing counter for profile IDs. Starts at 1.

#### `OwnerProfile`
```rust
pub type OwnerProfile<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ProfileId, OptionQuery>;
```
Maps each blockchain account to their profile ID. Enforces 1:1 relationship.

#### `ProfileOwner`
```rust
pub type ProfileOwner<T: Config> = StorageMap<_, Blake2_128Concat, ProfileId, T::AccountId, OptionQuery>;
```
Reverse mapping from profile ID to owner account. Used for ownership verification.

#### `HandleToProfile`
```rust
pub type HandleToProfile<T: Config> = StorageMap<_, Blake2_128Concat, HandleOf<T>, ProfileId, OptionQuery>;
```
Maps unique handles (usernames) to profile IDs. Enforces handle uniqueness.

#### `MetadataCid`
```rust
pub type MetadataCid<T: Config> = StorageMap<_, Blake2_128Concat, ProfileId, CidOf<T>, OptionQuery>;
```
Stores IPFS CID for each profile's metadata (avatar, bio, etc.).

### Reputation & Badges

#### `Reputation`
```rust
pub type Reputation<T: Config> = StorageMap<_, Blake2_128Concat, ProfileId, i32, ValueQuery>;
```
Current reputation score for each profile (can be negative). Default: 0.

Tier system:
- **Bronze**: 0-99 points
- **Prata**: 100-499 points
- **Ouro**: 500-999 points
- **Diamante**: 1000+ points

#### `Badges`
```rust
pub type Badges<T: Config> = StorageMap<_, Blake2_128Concat, ProfileId, BadgeListOf<T>, ValueQuery>;
```
Set of badges awarded to each profile. Max 50 badges per profile.

Badge structure:
```rust
pub struct Badge<MaxCodeLen: Get<u32>> {
    pub code: BoundedVec<u8, MaxCodeLen>,      // e.g., "verified_seller"
    pub issuer: ModuleId,                       // Which module issued it
    pub issued_at: u64,                         // Block number
    pub revoked_at: Option<u64>,                // Block number if revoked
}
```

#### `Penalties`
```rust
pub type Penalties<T: Config> = StorageMap<_, Blake2_128Concat, ProfileId, PenaltyListOf<T>, ValueQuery>;
```
List of penalties (violations) for each profile. Max 100 penalties.

Penalty structure:
```rust
pub struct Penalty<MaxReasonLen: Get<u32>> {
    pub id: u32,                                // Sequential ID
    pub reason: BoundedVec<u8, MaxReasonLen>,   // e.g., "spam"
    pub severity: u8,                           // 1-10 scale
    pub issued_at: u64,                         // Block number
    pub expires_at: Option<u64>,                // Block number
    pub revoked_at: Option<u64>,                // Block number if revoked
}
```

### History & Metadata

#### `HandleHistory`
```rust
pub type HandleHistory<T: Config> = StorageMap<_, Blake2_128Concat, ProfileId, HandleHistoryListOf<T>, ValueQuery>;
```
Historical record of handle changes. Max 20 records per profile.

HandleRecord structure:
```rust
pub struct HandleRecord<MaxHandleLen: Get<u32>> {
    pub handle: BoundedVec<u8, MaxHandleLen>,   // Previous handle
    pub changed_at: u64,                         // Block number
}
```

### Authorization & Control

#### `AuthorizedModules`
```rust
pub type AuthorizedModules<T: Config> = StorageValue<_, BoundedBTreeSet<ModuleId, T::MaxAuthorizedModules>, ValueQuery>;
```
Set of module IDs authorized to modify reputation. Root-controlled.

#### `AuthorizedIssuers`
```rust
pub type AuthorizedIssuers<T: Config> = StorageValue<_, BoundedBTreeSet<ModuleId, T::MaxAuthorizedModules>, ValueQuery>;
```
Set of module IDs authorized to award/revoke badges. Root-controlled.

#### `PenaltyRevokers`
```rust
pub type PenaltyRevokers<T: Config> = StorageValue<_, BoundedBTreeSet<ModuleId, T::MaxAuthorizedModules>, ValueQuery>;
```
Set of module IDs authorized to revoke penalties. Root-controlled.

#### `Paused`
```rust
pub type Paused<T> = StorageValue<_, bool, ValueQuery>;
```
Emergency pause flag. When true, all profile mutations are blocked.

## Configuration

```rust
impl pallet_bazari_identity::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxCidLen = ConstU32<96>;              // IPFS CID max length
    type MaxHandleLen = ConstU32<32>;            // Username max length
    type MaxBadgeCodeLen = ConstU32<32>;         // Badge code max length
    type MaxBadges = ConstU32<50>;               // Max badges per profile
    type MaxReasonCodeLen = ConstU32<64>;        // Reputation reason max length
    type MaxPenaltyReasonLen = ConstU32<128>;    // Penalty reason max length
    type MaxPenalties = ConstU32<100>;           // Max penalties per profile
    type MaxHandleHistory = ConstU32<20>;        // Max handle history records
    type MaxAuthorizedModules = ConstU32<100>;   // Max authorized modules
    type HandleCooldownBlocks = ConstU32<432000>; // 30 days (at 6s/block)
    type MintOrigin = EnsureRoot<AccountId>;     // Who can mint profiles
    type UpdateOrigin = EnsureRoot<AccountId>;   // Who can update metadata
    type ModuleOrigin = EnsureRoot<AccountId>;   // Who can modify reputation/badges
}
```

## Extrinsics

### `mint_profile`

Creates a new soulbound profile NFT for an account.

```rust
pub fn mint_profile(
    origin: OriginFor<T>,
    owner: T::AccountId,
    handle: Vec<u8>,
    cid: Vec<u8>
) -> DispatchResult
```

**Parameters:**
- `origin`: Must satisfy `MintOrigin` (typically sudo/root)
- `owner`: Account that will own this profile
- `handle`: Unique username (2-32 characters)
- `cid`: IPFS CID for profile metadata

**Errors:**
- `AlreadyHasProfile`: Account already owns a profile
- `HandleTaken`: Handle already in use
- `HandleTooLong`: Handle exceeds `MaxHandleLen`
- `CidTooLong`: CID exceeds `MaxCidLen`
- `Paused`: Pallet is paused

**Events:**
- `ProfileMinted { profile_id, owner, handle, cid }`

**Example (Polkadot.js):**
```javascript
const tx = api.tx.bazariIdentity.mintProfile(
  '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
  'alice',
  'bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi'
);
await tx.signAndSend(sudoAccount);
```

### `update_metadata_cid`

Updates the IPFS CID for a profile's metadata.

```rust
pub fn update_metadata_cid(
    origin: OriginFor<T>,
    profile_id: ProfileId,
    cid: Vec<u8>
) -> DispatchResult
```

**Parameters:**
- `origin`: Must satisfy `UpdateOrigin`
- `profile_id`: Target profile ID
- `cid`: New IPFS CID

**Errors:**
- `ProfileNotFound`: Profile ID doesn't exist
- `CidTooLong`: CID exceeds `MaxCidLen`
- `Paused`: Pallet is paused

**Events:**
- `MetadataUpdated { profile_id, cid }`

### `set_handle`

Changes a profile's handle (username). Subject to 30-day cooldown.

```rust
pub fn set_handle(
    origin: OriginFor<T>,
    profile_id: ProfileId,
    new_handle: Vec<u8>
) -> DispatchResult
```

**Parameters:**
- `origin`: Must be profile owner (signed)
- `profile_id`: Profile to update
- `new_handle`: New username

**Errors:**
- `ProfileNotFound`: Profile ID doesn't exist
- `NotProfileOwner`: Caller doesn't own this profile
- `HandleTaken`: New handle already in use
- `HandleCooldownActive`: Must wait 30 days since last change
- `HandleTooLong`: Handle exceeds `MaxHandleLen`
- `Paused`: Pallet is paused

**Events:**
- `HandleChanged { profile_id, old_handle, new_handle }`

**Cooldown Calculation:**
At 6 seconds per block, 30 days = 432,000 blocks.

### `increment_reputation`

Increases a profile's reputation score.

```rust
pub fn increment_reputation(
    origin: OriginFor<T>,
    profile_id: ProfileId,
    points: i32,
    reason_code: Vec<u8>
) -> DispatchResult
```

**Parameters:**
- `origin`: Must satisfy `ModuleOrigin` + be in `AuthorizedModules`
- `profile_id`: Target profile
- `points`: Points to add (must be positive)
- `reason_code`: Event code (e.g., "ORDER_COMPLETED")

**Errors:**
- `ProfileNotFound`: Profile ID doesn't exist
- `NotAuthorizedModule`: Caller not in authorized modules set
- `InvalidReputationPoints`: Points must be positive
- `ReputationCodeTooLong`: Reason code exceeds `MaxReasonCodeLen`
- `Paused`: Pallet is paused

**Events:**
- `ReputationChanged { profile_id, old_score, new_score, reason_code }`

### `decrement_reputation`

Decreases a profile's reputation score.

```rust
pub fn decrement_reputation(
    origin: OriginFor<T>,
    profile_id: ProfileId,
    points: i32,
    reason_code: Vec<u8>
) -> DispatchResult
```

Same parameters and errors as `increment_reputation`, but points are subtracted.

### `award_badge`

Awards a verifiable badge to a profile.

```rust
pub fn award_badge(
    origin: OriginFor<T>,
    profile_id: ProfileId,
    code: Vec<u8>,
    issuer: ModuleId
) -> DispatchResult
```

**Parameters:**
- `origin`: Must satisfy `ModuleOrigin` + be in `AuthorizedIssuers`
- `profile_id`: Target profile
- `code`: Badge code (e.g., "verified_seller")
- `issuer`: Module ID issuing the badge

**Errors:**
- `ProfileNotFound`: Profile ID doesn't exist
- `NotAuthorizedIssuer`: Caller not in authorized issuers set
- `BadgeCodeTooLong`: Code exceeds `MaxBadgeCodeLen`
- `BadgeAlreadyAwarded`: Profile already has this badge
- `TooManyBadges`: Profile has reached max badges limit
- `Paused`: Pallet is paused

**Events:**
- `BadgeAwarded { profile_id, code, issuer }`

### `revoke_badge`

Revokes a previously awarded badge.

```rust
pub fn revoke_badge(
    origin: OriginFor<T>,
    profile_id: ProfileId,
    code: Vec<u8>
) -> DispatchResult
```

**Parameters:**
- `origin`: Must satisfy `ModuleOrigin` + be in `AuthorizedIssuers`
- `profile_id`: Target profile
- `code`: Badge code to revoke

**Errors:**
- `ProfileNotFound`: Profile ID doesn't exist
- `NotAuthorizedIssuer`: Caller not in authorized issuers set
- `BadgeNotFound`: Profile doesn't have this badge
- `Paused`: Pallet is paused

**Events:**
- `BadgeRevoked { profile_id, code }`

### `add_penalty`

Records a penalty (violation) against a profile.

```rust
pub fn add_penalty(
    origin: OriginFor<T>,
    profile_id: ProfileId,
    reason: Vec<u8>,
    severity: u8,
    expires_at: Option<T::BlockNumber>
) -> DispatchResult
```

**Parameters:**
- `origin`: Must satisfy `ModuleOrigin`
- `profile_id`: Target profile
- `reason`: Description of violation (e.g., "spam")
- `severity`: 1-10 scale (10 = most severe)
- `expires_at`: Optional expiration block number

**Errors:**
- `ProfileNotFound`: Profile ID doesn't exist
- `PenaltyReasonTooLong`: Reason exceeds `MaxPenaltyReasonLen`
- `TooManyPenalties`: Profile has reached max penalties limit
- `Paused`: Pallet is paused

**Events:**
- `PenaltyAdded { profile_id, penalty_id, reason, severity }`

### `revoke_penalty`

Revokes a penalty (e.g., after successful appeal).

```rust
pub fn revoke_penalty(
    origin: OriginFor<T>,
    profile_id: ProfileId,
    penalty_id: u32
) -> DispatchResult
```

**Parameters:**
- `origin`: Must satisfy `ModuleOrigin` + be in `PenaltyRevokers`
- `profile_id`: Target profile
- `penalty_id`: Penalty ID to revoke

**Errors:**
- `ProfileNotFound`: Profile ID doesn't exist
- `NotAuthorizedRevoker`: Caller not in penalty revokers set
- `PenaltyNotFound`: Penalty ID doesn't exist for this profile
- `Paused`: Pallet is paused

**Events:**
- `PenaltyRevoked { profile_id, penalty_id }`

### `authorize_module`

Authorizes a module to modify reputation.

```rust
pub fn authorize_module(
    origin: OriginFor<T>,
    module_id: ModuleId
) -> DispatchResult
```

**Parameters:**
- `origin`: Must be root
- `module_id`: Module ID to authorize (e.g., 1 = marketplace)

**Events:**
- `ModuleAuthorized { module_id }`

### `authorize_issuer`

Authorizes a module to award/revoke badges.

```rust
pub fn authorize_issuer(
    origin: OriginFor<T>,
    module_id: ModuleId
) -> DispatchResult
```

**Parameters:**
- `origin`: Must be root
- `module_id`: Module ID to authorize

**Events:**
- `IssuerAuthorized { module_id }`

### `set_paused`

Emergency pause/unpause the pallet.

```rust
pub fn set_paused(
    origin: OriginFor<T>,
    paused: bool
) -> DispatchResult
```

**Parameters:**
- `origin`: Must be root
- `paused`: `true` to pause, `false` to unpause

**Events:**
- `PausedChanged { paused }`

## Usage Examples

### TypeScript (via @polkadot/api)

```typescript
import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';

// Connect to node
const wsProvider = new WsProvider('ws://127.0.0.1:9944');
const api = await ApiPromise.create({ provider: wsProvider });

// Mint a profile
const keyring = new Keyring({ type: 'sr25519' });
const sudo = keyring.addFromUri('//Alice');

const mintTx = api.tx.bazariIdentity.mintProfile(
  '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
  'alice',
  'bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi'
);

await mintTx.signAndSend(sudo, ({ status, events }) => {
  if (status.isInBlock) {
    events.forEach(({ event }) => {
      if (api.events.bazariIdentity.ProfileMinted.is(event)) {
        const [profileId] = event.data;
        console.log(`Profile minted: #${profileId}`);
      }
    });
  }
});

// Query profile by handle
const profileId = await api.query.bazariIdentity.handleToProfile('alice');
if (profileId.isSome) {
  const owner = await api.query.bazariIdentity.profileOwner(profileId.unwrap());
  const reputation = await api.query.bazariIdentity.reputation(profileId.unwrap());
  const cid = await api.query.bazariIdentity.metadataCid(profileId.unwrap());

  console.log(`Profile #${profileId.unwrap()}`);
  console.log(`Owner: ${owner.unwrap()}`);
  console.log(`Reputation: ${reputation}`);
  console.log(`Metadata: ${cid.unwrap()}`);
}

// Award a badge (as authorized module)
const awardTx = api.tx.bazariIdentity.awardBadge(
  1,  // profile_id
  'verified_seller',
  1   // module_id (marketplace)
);
await awardTx.signAndSend(sudo);

// Increment reputation
const repTx = api.tx.bazariIdentity.incrementReputation(
  1,  // profile_id
  10, // points
  'ORDER_COMPLETED'
);
await repTx.signAndSend(sudo);
```

### Polkadot.js Apps UI

1. Navigate to **Developer → Extrinsics**
2. Select **bazariIdentity** pallet
3. Choose extrinsic (e.g., `mintProfile`)
4. Fill parameters:
   - `owner`: Select account from dropdown
   - `handle`: Enter username
   - `cid`: Paste IPFS CID
5. Submit with sudo account

To query storage:
1. Navigate to **Developer → Chain State**
2. Select **bazariIdentity** pallet
3. Choose storage item (e.g., `handleToProfile`)
4. Enter key and click **+**

## Events

All events emitted by this pallet:

```rust
pub enum Event<T: Config> {
    ProfileMinted { profile_id: ProfileId, owner: T::AccountId, handle: Vec<u8>, cid: Vec<u8> },
    MetadataUpdated { profile_id: ProfileId, cid: Vec<u8> },
    HandleChanged { profile_id: ProfileId, old_handle: Vec<u8>, new_handle: Vec<u8> },
    ReputationChanged { profile_id: ProfileId, old_score: i32, new_score: i32, reason_code: Vec<u8> },
    BadgeAwarded { profile_id: ProfileId, code: Vec<u8>, issuer: ModuleId },
    BadgeRevoked { profile_id: ProfileId, code: Vec<u8> },
    PenaltyAdded { profile_id: ProfileId, penalty_id: u32, reason: Vec<u8>, severity: u8 },
    PenaltyRevoked { profile_id: ProfileId, penalty_id: u32 },
    ModuleAuthorized { module_id: ModuleId },
    IssuerAuthorized { module_id: ModuleId },
    PausedChanged { paused: bool },
}
```

## Testing

### Run Unit Tests

```bash
cd pallets/bazari-identity
cargo test
```

### Test Coverage

The pallet includes 15 comprehensive tests:

- ✅ `mint_profile_creates_new_profile`
- ✅ `mint_profile_fails_if_already_exists`
- ✅ `mint_profile_fails_if_handle_taken`
- ✅ `update_metadata_cid_works`
- ✅ `set_handle_works_after_cooldown`
- ✅ `set_handle_fails_during_cooldown`
- ✅ `increment_reputation_works_for_authorized_module`
- ✅ `increment_reputation_fails_for_unauthorized_module`
- ✅ `decrement_reputation_works`
- ✅ `award_badge_works_for_authorized_issuer`
- ✅ `award_badge_fails_for_duplicate`
- ✅ `revoke_badge_works`
- ✅ `add_penalty_works`
- ✅ `revoke_penalty_works`
- ✅ `pause_blocks_all_mutations`

### Integration Testing

See [E2E test suite](../../docs/testing/profile-nft-e2e.md) for full integration tests with backend API.

## Security Considerations

### Soulbound Properties

- **Non-transferable**: Profiles cannot be transferred between accounts
- **1:1 Guarantee**: Each account can only own one profile
- **Immutable Owner**: Profile ownership cannot be changed (except via chain upgrade)

### Access Control

- **Minting**: Restricted to `MintOrigin` (typically sudo during initial deployment)
- **Reputation**: Only authorized modules can modify scores
- **Badges**: Only authorized issuers can award/revoke badges
- **Penalties**: Only authorized revokers can revoke penalties
- **Emergency Pause**: Root can pause all mutations for security incidents

### Data Integrity

- **Handle Uniqueness**: Enforced via `HandleToProfile` storage map
- **Cooldown Prevention**: 30-day cooldown prevents handle spam/squatting
- **Bounded Collections**: All collections have max limits to prevent storage bloat
- **Immutable History**: Handle history and penalty records are append-only

## Migration Guide

### From v0.1.0 to v1.0.0 (Future)

If you're upgrading from the initial version, run this migration:

```rust
pub mod v1 {
    use super::*;
    use frame_support::traits::OnRuntimeUpgrade;

    pub struct MigrateToV1<T>(PhantomData<T>);

    impl<T: Config> OnRuntimeUpgrade for MigrateToV1<T> {
        fn on_runtime_upgrade() -> Weight {
            // Add migration logic here
            T::DbWeight::get().reads_writes(0, 0)
        }
    }
}
```

## License

MIT-0

## Support

- **Issues**: https://github.com/bazari/bazari-chain/issues
- **Docs**: https://docs.bazari.dev
- **Discord**: https://discord.gg/bazari
