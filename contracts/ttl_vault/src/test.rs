#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    (env, owner, beneficiary)
}

#[test]
fn test_create_vault() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    assert_eq!(vault_id, 1);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, owner);
    assert_eq!(vault.beneficiary, beneficiary);
    assert_eq!(vault.balance, 0);
}

#[test]
fn test_check_in_resets_timer() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);

    // Advance time by 12 hours
    env.ledger().with_mut(|l| l.timestamp += 43200);
    client.check_in(&vault_id);

    // TTL remaining should be close to full interval again
    let remaining = client.get_ttl_remaining(&vault_id);
    assert!(remaining > 43000 && remaining <= 86400);
}

#[test]
fn test_is_not_expired_before_interval() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    env.ledger().with_mut(|l| l.timestamp += 43200);

    assert!(!client.is_expired(&vault_id));
}

#[test]
fn test_is_expired_after_interval() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    env.ledger().with_mut(|l| l.timestamp += 90000); // past 24h

    assert!(client.is_expired(&vault_id));
}

#[test]
fn test_update_beneficiary() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    let new_beneficiary = Address::generate(&env);
    client.update_beneficiary(&vault_id, &new_beneficiary);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.beneficiary, new_beneficiary);
}

#[test]
#[should_panic(expected = "vault already released")]
fn test_update_beneficiary_after_release_fails() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);

    // Advance time past the check-in interval to expire the vault
    env.ledger().with_mut(|l| l.timestamp += 90000);
    client.trigger_release(&vault_id);

    // Attempt to update beneficiary on a Released vault — must panic
    let new_beneficiary = Address::generate(&env);
    client.update_beneficiary(&vault_id, &new_beneficiary);
}

#[test]
fn test_update_beneficiary_while_locked_near_expiry() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);

    // Advance time to just before expiry — vault is still Locked
    env.ledger().with_mut(|l| l.timestamp += 86399);
    assert!(!client.is_expired(&vault_id));

    let new_beneficiary = Address::generate(&env);
    client.update_beneficiary(&vault_id, &new_beneficiary);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.beneficiary, new_beneficiary);
}
