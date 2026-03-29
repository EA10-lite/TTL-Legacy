#![cfg(test)]

use super::*;
use soroban_sdk::{bytes, Env};

#[test]
fn test_verify_claim_succeeds_with_nonempty_proof_and_claim() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ZkVerifierContract);
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);

    assert!(client.verify_claim(&proof, &claim));
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_verify_claim_rejects_empty_proof() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ZkVerifierContract);
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let proof = bytes!(&env,);
    let claim = bytes!(&env, 0xcafebabe);

    client.verify_claim(&proof, &claim);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_verify_claim_rejects_empty_claim() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ZkVerifierContract);
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env,);

    client.verify_claim(&proof, &claim);
}
