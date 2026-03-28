#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{self, StellarAssetClient},
    vec, Address, BytesN, Env,
};

fn setup() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, admin, token_address, client)
}

// ---- existing tests ----

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_guard_against_double_init() {
    let (env, _, _, admin, token_address, client) = setup();

    let new_token_admin = Address::generate(&env);
    let new_token_address = env
        .register_stellar_asset_contract_v2(new_token_admin)
        .address();

    client.initialize(&new_token_address, &admin);
    let _ = token_address;
}

#[test]
fn test_vault_count_view() {
    let (_, owner, beneficiary, _, _, client) = setup();

    assert_eq!(client.vault_count(), 0);
    let id_1 = client.create_vault(&owner, &beneficiary, &100u64);
    let id_2 = client.create_vault(&owner, &beneficiary, &200u64);

    assert_eq!(id_1, 1);
    assert_eq!(id_2, 2);
    assert_eq!(client.vault_count(), 2);
}

#[test]
fn test_vault_exists_for_existing_and_missing_ids() {
    let (_, owner, beneficiary, _, _, client) = setup();

    assert!(!client.vault_exists(&1));

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

    assert!(client.vault_exists(&vault_id));
    assert!(!client.vault_exists(&(vault_id + 1)));
}

#[test]
fn test_get_release_status_view() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Locked);

    client.deposit(&vault_id, &owner, &500i128);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    assert_eq!(
        client.get_release_status(&vault_id),
        ReleaseStatus::Released
    );

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 500i128);
}

#[test]
fn test_batch_deposit_updates_multiple_vaults() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64);
    let token_client = token::Client::new(&env, &token_address);

    client.batch_deposit(
        &owner,
        &vec![&env, (vault_id_1, 150i128), (vault_id_2, 250i128)],
    );

    assert_eq!(client.get_vault(&vault_id_1).balance, 150i128);
    assert_eq!(client.get_vault(&vault_id_2).balance, 250i128);
    assert_eq!(token_client.balance(&owner), 999_600i128);
}

#[test]
fn test_batch_deposit_validates_all_items_before_transfer() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    let token_client = token::Client::new(&env, &token_address);

    assert!(
        client
            .try_batch_deposit(&owner, &vec![&env, (vault_id, 100i128), (999u64, 200i128)])
            .is_err()
    );

    assert_eq!(client.get_vault(&vault_id).balance, 0i128);
    assert_eq!(token_client.balance(&owner), 1_000_000i128);

    assert!(
        client
            .try_batch_deposit(&owner, &vec![&env, (vault_id, 100i128), (vault_id, 0i128)])
            .is_err()
    );

    assert_eq!(client.get_vault(&vault_id).balance, 0i128);
    assert_eq!(token_client.balance(&owner), 1_000_000i128);
}

#[test]
fn test_pause_and_unpause_toggle() {
    let (_, _, _, _, _, client) = setup();

    assert!(!client.is_paused());
    client.pause();
    assert!(client.is_paused());
    client.unpause();
    assert!(!client.is_paused());
}

#[test]
fn test_get_admin_view() {
    let (_, _, _, admin, _, client) = setup();

    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_paused_blocks_check_in_withdraw_and_trigger_release() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &200i128);
    env.ledger().with_mut(|l| l.timestamp += 200);

    client.pause();

    assert!(client.try_check_in(&vault_id, &owner).is_err());
    assert!(client.try_withdraw(&vault_id, &10i128).is_err());
    assert!(client.try_trigger_release(&vault_id).is_err());

    client.unpause();
    client.trigger_release(&vault_id);
    assert_eq!(
        client.get_release_status(&vault_id),
        ReleaseStatus::Released
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_create_vault_rejects_owner_as_beneficiary() {
    let (_, owner, _, _, _, client) = setup();
    client.create_vault(&owner, &owner, &1000);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_update_beneficiary_rejects_owner_as_beneficiary() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &1000);
    client.update_beneficiary(&vault_id, &owner);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_deposit_into_expired_vault_is_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.deposit(&vault_id, &owner, &500i128);
}

#[test]
fn test_trigger_release_emits_event_with_zero_balance() {
    let (env, owner, beneficiary, _, _, client) = setup();

    // create vault but never deposit — balance stays 0
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    env.ledger().with_mut(|l| l.timestamp += 200);

    // should succeed and emit a release event with amount: 0
    client.trigger_release(&vault_id);

    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Released);

    let events = env.events().all();
    let release_event = events.iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        if topics.len() < 1 {
            return false;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        topic0.map(|s| s == soroban_sdk::symbol_short!("release")).unwrap_or(false)
    });

    assert!(release_event.is_some(), "release event not emitted for zero-balance vault");
}
