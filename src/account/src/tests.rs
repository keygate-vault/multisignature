use b3_utils::{ledger::ICRCAccount, Subaccount};
use candid::{encode_one, CandidType, Decode, Principal};
use ic_ledger_types::{AccountIdentifier, Tokens};
use icrc_ledger_types::icrc1::account::Account;
#[cfg(test)]
use pocket_ic::PocketIc;
#[cfg(test)]
use pocket_ic::WasmResult;
#[cfg(test)]
use pocket_ic::{query_candid_as, update_candid, update_candid_as};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::{error::Error, fmt::format, io::Write};

#[cfg(test)]
use crate::to_subaccount;
use crate::types::{ArchiveOptions, FeatureFlags, ICRC1Args, ICRC1InitArgs};
use crate::{
    IntentStatus, ProposeTransactionArgs, ProposedTransaction, SupportedNetwork,
    TransactionRequest, TransactionType,
};

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize, Serialize)]
pub struct NnsLedgerCanisterInitPayload {
    pub minting_account: String,
    pub icrc1_minting_account: Option<Account>,
    pub initial_values: Vec<(String, Tokens)>,
    pub max_message_size_bytes: Option<usize>,
    pub transaction_window: Option<Duration>,
    pub archive_options: Option<ArchiveOptions>,
    pub send_whitelist: HashSet<Principal>,
    pub transfer_fee: Option<Tokens>,
    pub token_symbol: Option<String>,
    pub token_name: Option<String>,
    pub feature_flags: Option<FeatureFlags>,
    pub maximum_number_of_accounts: Option<usize>,
    pub accounts_overflow_trim_quantity: Option<usize>,
}

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize, Serialize)]
pub struct NnsLedgerCanisterUpgradePayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icrc1_minting_account: Option<Account>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_flags: Option<FeatureFlags>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize, Serialize)]
pub enum LedgerCanisterPayload {
    Init(NnsLedgerCanisterInitPayload),
    Upgrade(Option<NnsLedgerCanisterUpgradePayload>),
}

#[cfg(test)]
pub fn get_icp_balance(env: &PocketIc, user_id: Principal) -> u64 {
    use ic_ledger_types::{AccountBalanceArgs, Tokens, DEFAULT_SUBACCOUNT};
    use pocket_ic::update_candid_as;

    let ledger_canister_id = Principal::from_text("ryjl3-tyaaa-aaaaa-aaaba-cai").unwrap();
    let account = AccountIdentifier::new(&user_id, &DEFAULT_SUBACCOUNT);
    let account_balance_args = AccountBalanceArgs { account };
    let res: (Tokens,) = update_candid_as(
        env,
        ledger_canister_id,
        user_id,
        "account_balance",
        (account_balance_args,),
    )
    .unwrap();
    res.0.e8s()
}

#[cfg(test)]
fn generate_principal() -> Principal {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    Principal::self_authenticating(&verifying_key.as_bytes())
}

#[cfg(test)]
fn gzip(blob: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    use libflate::gzip::Encoder;
    let mut encoder = Encoder::new(Vec::with_capacity(blob.len())).unwrap();
    encoder.write_all(&blob)?;
    Ok(encoder.finish().into_result().unwrap())
}

#[test]
fn should_initialize_with_default_values() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    pic.add_cycles(account_id, 2_000_000_000_000);
    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let wasm_result = pic.query_call(account_id, caller, "get_signers", encode_one(()).unwrap());
    match wasm_result.unwrap() {
        pocket_ic::WasmResult::Reject(reject_message) => {
            panic!("Query call failed: {}", reject_message);
        }
        pocket_ic::WasmResult::Reply(reply) => {
            let signers = Decode!(&reply, Vec<Principal>);

            // caller should be included in signers vector
            match signers {
                Ok(signers) => assert_eq!(signers, vec![caller]),
                Err(e) => panic!("Error decoding signers: {}", e),
            }
        }
    }

    // check if supported blockchain adapters have icp:native:transfer
    let wasm_result = pic.query_call(
        account_id,
        caller,
        "get_supported_blockchain_adapters",
        encode_one(()).unwrap(),
    );

    match wasm_result.unwrap() {
        pocket_ic::WasmResult::Reject(reject_message) => {
            panic!("Query call failed: {}", reject_message);
        }
        pocket_ic::WasmResult::Reply(reply) => {
            let adapters = Decode!(&reply, Vec<String>);

            match adapters {
                Ok(adapters) => assert!(adapters.contains(&"icp:native:transfer".to_string())),
                Err(e) => panic!("Error decoding adapters: {}", e),
            }
        }
    }
}

#[test]
fn should_add_signer() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    pic.add_cycles(account_id, 2_000_000_000_000);
    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    let signer = generate_principal();

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let wasm_result = pic.update_call(
        account_id,
        caller,
        "add_signer",
        encode_one(signer).unwrap(),
    );

    match wasm_result.unwrap() {
        pocket_ic::WasmResult::Reject(reject_message) => {
            panic!("Update call failed: {}", reject_message);
        }
        pocket_ic::WasmResult::Reply(_) => {
            let wasm_result =
                pic.query_call(account_id, caller, "get_signers", encode_one(()).unwrap());
            match wasm_result.unwrap() {
                pocket_ic::WasmResult::Reject(reject_message) => {
                    panic!("Query call failed: {}", reject_message);
                }
                pocket_ic::WasmResult::Reply(reply) => {
                    let signers = Decode!(&reply, Vec<Principal>);

                    // caller should be included in signers vector
                    match signers {
                        Ok(signers) => assert_eq!(signers, vec![caller, signer]),
                        Err(e) => panic!("Error decoding signers: {}", e),
                    }
                }
            }
        }
    }
}

#[test]
fn should_propose_transaction() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    pic.add_cycles(account_id, 2_000_000_000_000);

    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let proposed_transaction: (ProposedTransaction,) = update_candid_as(
        &pic,
        account_id,
        caller,
        "propose_transaction",
        (ProposeTransactionArgs {
            to: "test".to_string(),
            token: "test".to_string(),
            network: SupportedNetwork::ICP,
            amount: 100_000_000.0,
            transaction_type: TransactionType::Transfer,
        },),
    )
    .unwrap();

    assert_eq!(proposed_transaction.0.id, 0);
    assert_eq!(proposed_transaction.0.to, "test");
    assert_eq!(proposed_transaction.0.token, "test");
    assert_eq!(proposed_transaction.0.network, SupportedNetwork::ICP);
    assert_eq!(proposed_transaction.0.amount, 100_000_000.0);
    assert_eq!(
        proposed_transaction.0.transaction_type,
        TransactionType::Transfer
    );
    assert_eq!(proposed_transaction.0.signers, vec![caller]);
}

#[test]
fn should_get_proposed_transaction() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    pic.add_cycles(account_id, 2_000_000_000_000);

    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let proposed_transaction: (ProposedTransaction,) = update_candid_as(
        &pic,
        account_id,
        caller,
        "propose_transaction",
        (ProposeTransactionArgs {
            to: "test".to_string(),
            token: "test".to_string(),
            network: SupportedNetwork::ICP,
            amount: 100_000_000.0,
            transaction_type: TransactionType::Transfer,
        },),
    )
    .unwrap();

    let proposed_transaction_2: (Option<ProposedTransaction>,) = query_candid_as(
        &pic,
        account_id,
        caller,
        "get_proposed_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    match proposed_transaction_2.0 {
        Some(proposed_transaction_2) => assert_eq!(proposed_transaction.0, proposed_transaction_2),
        None => panic!("Proposed transaction not found"),
    }
}

#[test]
fn should_get_proposed_transactions() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    pic.add_cycles(account_id, 2_000_000_000_000);

    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let proposed_transaction: (ProposedTransaction,) = update_candid_as(
        &pic,
        account_id,
        caller,
        "propose_transaction",
        (ProposeTransactionArgs {
            to: "test".to_string(),
            token: "test".to_string(),
            network: SupportedNetwork::ICP,
            amount: 100_000_000.0,
            transaction_type: TransactionType::Transfer,
        },),
    )
    .unwrap();

    let proposed_transactions: (Vec<ProposedTransaction>,) =
        query_candid_as(&pic, account_id, caller, "get_proposed_transactions", ()).unwrap();

    assert_eq!(proposed_transactions.0.len(), 1);
    assert_eq!(proposed_transactions.0[0], proposed_transaction.0);
}

#[test]
fn should_approve_transaction() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    pic.add_cycles(account_id, 2_000_000_000_000);

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let proposed_transaction: (ProposedTransaction,) = update_candid_as(
        &pic,
        account_id,
        caller,
        "propose_transaction",
        (ProposeTransactionArgs {
            to: "test".to_string(),
            token: "test".to_string(),
            network: SupportedNetwork::ICP,
            amount: 100_000_000.0,
            transaction_type: TransactionType::Transfer,
        },),
    )
    .unwrap();

    let signer_2 = generate_principal();

    let signer_3 = generate_principal();

    let r1: () = update_candid_as(&pic, account_id, caller, "add_signer", (signer_2,)).unwrap();

    let r2: () = update_candid_as(&pic, account_id, caller, "add_signer", (signer_3,)).unwrap();

    let approve_result: () = update_candid_as(
        &pic,
        account_id,
        signer_2,
        "approve_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    let proposed_transaction_2: (Option<ProposedTransaction>,) = query_candid_as(
        &pic,
        account_id,
        caller,
        "get_proposed_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    match proposed_transaction_2.0 {
        Some(proposed_transaction_2) => {
            assert_eq!(proposed_transaction_2.signers, vec![caller, signer_2])
        }
        None => panic!("Proposed transaction not found"),
    }

    let approve_result_2: () = update_candid_as(
        &pic,
        account_id,
        signer_3,
        "approve_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    let proposed_transaction_3: (Option<ProposedTransaction>,) = query_candid_as(
        &pic,
        account_id,
        caller,
        "get_proposed_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    match proposed_transaction_3.0 {
        Some(proposed_transaction_3) => assert_eq!(
            proposed_transaction_3.signers,
            vec![caller, signer_2, signer_3]
        ),
        None => panic!("Proposed transaction not found"),
    }
}

#[test]
fn should_reject_transaction() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    pic.add_cycles(account_id, 2_000_000_000_000);

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let proposed_transaction: (ProposedTransaction,) = update_candid_as(
        &pic,
        account_id,
        caller,
        "propose_transaction",
        (ProposeTransactionArgs {
            to: "test".to_string(),
            token: "test".to_string(),
            network: SupportedNetwork::ICP,
            amount: 100_000_000.0,
            transaction_type: TransactionType::Transfer,
        },),
    )
    .unwrap();

    let signer_2 = generate_principal();
    let signer_3 = generate_principal();

    let r1: () = update_candid_as(&pic, account_id, caller, "add_signer", (signer_2,)).unwrap();
    let r2: () = update_candid_as(&pic, account_id, caller, "add_signer", (signer_3,)).unwrap();

    let reject_result: () = update_candid_as(
        &pic,
        account_id,
        signer_2,
        "reject_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    let proposed_transaction_2: (Option<ProposedTransaction>,) = query_candid_as(
        &pic,
        account_id,
        caller,
        "get_proposed_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    match proposed_transaction_2.0 {
        Some(proposed_transaction_2) => {
            assert_eq!(proposed_transaction_2.rejections, vec![signer_2])
        }
        None => panic!("Proposed transaction not found"),
    }

    let reject_result_2: () = update_candid_as(
        &pic,
        account_id,
        signer_3,
        "reject_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    let proposed_transaction_3: (Option<ProposedTransaction>,) = query_candid_as(
        &pic,
        account_id,
        caller,
        "get_proposed_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    match proposed_transaction_3.0 {
        Some(proposed_transaction_3) => {
            assert_eq!(proposed_transaction_3.rejections, vec![signer_2, signer_3])
        }
        None => panic!("Proposed transaction not found"),
    }
}

#[test]
fn should_set_threshold() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    pic.add_cycles(account_id, 2_000_000_000_000);

    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let result: () = update_candid_as(
        &pic,
        account_id,
        caller,
        "set_threshold",
        (100_000_000 as u64,),
    )
    .unwrap();

    let threshold: (u64,) = query_candid_as(&pic, account_id, caller, "get_threshold", ()).unwrap();

    assert_eq!(threshold.0, 100_000_000);
}

#[test]
fn should_not_allow_tx_if_threshold_not_met() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    // Create and set up the account canister
    let account_id = pic.create_canister_with_settings(Some(caller), None);
    pic.add_cycles(account_id, 2_000_000_000_000);

    // Create and set up the ICRC ledger canister
    let icrc_ledger = pic.create_canister_with_settings(Some(caller), None);
    pic.add_cycles(icrc_ledger, 2_000_000_000_000);

    let icrc_wasm_module = include_bytes!("../../../mock_icrc1_wasm_build.gz").to_vec();

    // Initialize ICRC ledger with tokens for the account canister
    let mint_amount_u64: u128 = 1000_000_000_000;
    let icrc1_deploy_args = ICRC1Args::Init(ICRC1InitArgs {
        token_symbol: "MCK".to_string(),
        token_name: "Mock Token".to_string(),
        minting_account: Account {
            owner: caller,
            subaccount: None,
        },
        transfer_fee: 1_000_000,
        metadata: vec![],
        initial_balances: vec![(
            Account {
                owner: account_id,
                subaccount: None,
            },
            mint_amount_u64,
        )],
        archive_options: ArchiveOptions {
            num_blocks_to_archive: 10,
            trigger_threshold: 5,
            controller_id: account_id,
            max_transactions_per_response: None,
            max_message_size_bytes: None,
            cycles_for_archive_creation: None,
            node_max_memory_size_bytes: None,
        },
        feature_flags: Some(FeatureFlags { icrc2: false }),
        decimals: Some(3),
        maximum_number_of_accounts: None,
        accounts_overflow_trim_quantity: None,
        fee_collector_account: None,
        max_memo_length: None,
    });

    // Install ICRC ledger
    pic.install_canister(
        icrc_ledger,
        icrc_wasm_module,
        encode_one(icrc1_deploy_args).unwrap(),
        Some(caller),
    );

    // Install account canister
    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();
    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    // Add additional signers
    let signer_2 = generate_principal();
    let signer_3 = generate_principal();
    let _: () = update_candid_as(&pic, account_id, caller, "add_signer", (signer_2,)).unwrap();
    let _: () = update_candid_as(&pic, account_id, caller, "add_signer", (signer_3,)).unwrap();

    // Set threshold to require at least 2 signers
    let _: () = update_candid_as(&pic, account_id, caller, "set_threshold", (2u64,)).unwrap();

    // Generate a receiver for the transfer attempt
    let receiver = generate_principal();

    // Propose a transaction
    let proposed_transaction: (ProposedTransaction,) = update_candid_as(
        &pic,
        account_id,
        caller,
        "propose_transaction",
        (ProposeTransactionArgs {
            to: receiver.to_text(),
            token: format!("icp:icrc1:{}", icrc_ledger.to_text()),
            network: SupportedNetwork::ICP,
            amount: 100_000_000_000.0,
            transaction_type: TransactionType::Transfer,
        },),
    )
    .unwrap();

    // Try to execute with only one signer (should fail)
    let execute_result = pic.update_call(
        account_id,
        caller,
        "execute_transaction",
        encode_one(proposed_transaction.0.id).unwrap(),
    );

    // Verify that the execution was rejected due to threshold not being met
    match execute_result.unwrap() {
        WasmResult::Reply(reply) => {
            let status = Decode!(&reply, IntentStatus).unwrap();
            assert!(
                matches!(status.clone(), IntentStatus::Failed(msg) if msg.contains("Threshold not met")),
                "Expected failure due to threshold not met, found: {:?}",
                status.clone()
            );
        }
        WasmResult::Reject(msg) => panic!("Unexpected rejection: {}", msg),
    }

    // Verify receiver balance is still 0
    let balance: (u128,) = query_candid_as(
        &pic,
        icrc_ledger,
        caller,
        "icrc1_balance_of",
        (ICRCAccount::new(receiver, None),),
    )
    .unwrap();

    assert_eq!(
        balance.0, 0,
        "Receiver balance should be 0 as transfer should have failed"
    );

    // Have second signer approve the transaction
    let _: () = update_candid_as(
        &pic,
        account_id,
        signer_2,
        "approve_transaction",
        (proposed_transaction.0.id,),
    )
    .unwrap();

    // Now try executing the transaction again with two signers (should succeed)
    let execute_result = pic.update_call(
        account_id,
        caller,
        "execute_transaction",
        encode_one(proposed_transaction.0.id).unwrap(),
    );

    // Verify successful execution
    match execute_result.unwrap() {
        WasmResult::Reply(reply) => {
            let status = Decode!(&reply, IntentStatus).unwrap();
            assert!(
                matches!(status, IntentStatus::Completed(_)),
                "Expected successful completion after meeting threshold"
            );
        }
        WasmResult::Reject(msg) => panic!("Unexpected rejection after meeting threshold: {}", msg),
    }

    // Verify the transfer actually occurred
    let balance: (u128,) = query_candid_as(
        &pic,
        icrc_ledger,
        caller,
        "icrc1_balance_of",
        (ICRCAccount::new(receiver, None),),
    )
    .unwrap();

    assert_eq!(
        balance.0, 100_000_000_000,
        "Receiver should have received the transfer amount"
    );
}

#[test]
fn should_not_add_signer_if_exists() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    pic.add_cycles(account_id, 2_000_000_000_000);

    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    let signer = generate_principal();

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let wasm_result = pic.update_call(
        account_id,
        caller,
        "add_signer",
        encode_one(signer).unwrap(),
    );

    match wasm_result.unwrap() {
        pocket_ic::WasmResult::Reject(reject_message) => {
            panic!("Update call failed: {}", reject_message);
        }
        pocket_ic::WasmResult::Reply(_) => {
            let wasm_result = pic.update_call(
                account_id,
                caller,
                "add_signer",
                encode_one(signer).unwrap(),
            );

            match wasm_result.unwrap() {
                pocket_ic::WasmResult::Reject(reject_message) => {
                    assert_eq!(reject_message, "signer already exists");
                }
                pocket_ic::WasmResult::Reply(bytes) => {
                    let reply = Decode!(&bytes, String);

                    assert!(reply.is_err())
                }
            }
        }
    }
}

#[test]
fn should_get_default_account_for_icp() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    let subaccountid = AccountIdentifier::new(&account_id, &to_subaccount(0)).to_hex();
    pic.add_cycles(account_id, 2_000_000_000_000);

    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let wasm_result = pic.query_call(
        account_id,
        caller,
        "get_icp_account",
        encode_one(()).unwrap(),
    );

    match wasm_result.unwrap() {
        pocket_ic::WasmResult::Reject(reject_message) => {
            panic!("Query call failed: {}", reject_message);
        }
        pocket_ic::WasmResult::Reply(reply) => {
            println!("{:?}", reply);

            let account = Decode!(&reply, String);

            match account {
                Ok(y_account) => assert_eq!(y_account, subaccountid),
                Err(e) => panic!("Error decoding account: {}", e),
            }
        }
    }
}

#[test]
fn should_get_default_account_for_icrc() {
    let pic = PocketIc::new();
    let caller = generate_principal();

    let account_id = pic.create_canister_with_settings(Some(caller), None);

    let subaccount = to_subaccount(0);

    let subaccountid = ICRCAccount::new(account_id, None).to_text();

    pic.add_cycles(account_id, 2_000_000_000_000);

    let wasm_module =
        include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();

    pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

    let wasm_result = pic.query_call(
        account_id,
        caller,
        "get_icrc_account",
        encode_one(()).unwrap(),
    );

    match wasm_result.unwrap() {
        pocket_ic::WasmResult::Reject(reject_message) => {
            panic!("Query call failed: {}", reject_message);
        }
        pocket_ic::WasmResult::Reply(reply) => {
            let account = Decode!(&reply, String);

            match account {
                Ok(y_account) => assert_eq!(y_account, subaccountid),
                Err(e) => panic!("Error decoding account: {}", e),
            }
        }
    }
}

#[cfg(test)]
mod intent_tests {
    use std::collections::{HashMap, HashSet};

    use ic_ledger_types::{AccountBalanceArgs, Tokens, DEFAULT_SUBACCOUNT};
    use icrc_ledger_types::icrc1::account::Account;
    use num_bigint::ToBigUint;
    use pocket_ic::{common::rest::base64, query_candid, PocketIcBuilder, WasmResult};

    use crate::{
        ledger,
        types::{ArchiveOptions, FeatureFlags, ICRC1Args, ICRC1InitArgs},
        Intent, IntentStatus, SupportedNetwork, TransactionRequest, TransactionType,
        RECOMMENDED_ICP_TRANSACTION_FEE,
    };

    use super::*;

    #[test]
    fn should_transfer_icrc1() {
        let pic = PocketIcBuilder::new().with_application_subnet().build();
        let caller = generate_principal();

        // Create and set up the account canister
        let account_id = pic.create_canister_with_settings(Some(caller), None);
        pic.add_cycles(account_id, 2_000_000_000_000);
        let wasm_module =
            include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();
        pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

        // Create a receiver principal
        let receiver = generate_principal();

        // Set up the ICRC1 token canister
        let icrc_ledger = pic.create_canister_with_settings(Some(caller), None);
        pic.add_cycles(icrc_ledger, 2_000_000_000_000);
        let icrc_wasm_module = include_bytes!("../../../mock_icrc1_wasm_build.gz").to_vec();

        // Initialize ICRC1 ledger with initial balances and settings
        let mint_amount_u64: u128 = 1000_000_000_000;
        let icrc1_deploy_args = ICRC1Args::Init(ICRC1InitArgs {
            token_symbol: "MCK".to_string(),
            token_name: "Mock Token".to_string(),
            minting_account: Account {
                owner: caller,
                subaccount: None,
            },
            transfer_fee: 1_000_000,
            metadata: vec![],
            initial_balances: vec![(
                Account {
                    owner: account_id,
                    subaccount: None,
                },
                mint_amount_u64,
            )],
            archive_options: ArchiveOptions {
                num_blocks_to_archive: 10,
                trigger_threshold: 5,
                controller_id: account_id,
                max_transactions_per_response: None,
                max_message_size_bytes: None,
                cycles_for_archive_creation: None,
                node_max_memory_size_bytes: None,
            },
            feature_flags: Some(FeatureFlags { icrc2: false }),
            decimals: Some(3),
            maximum_number_of_accounts: None,
            accounts_overflow_trim_quantity: None,
            fee_collector_account: None,
            max_memo_length: None,
        });

        pic.install_canister(
            icrc_ledger,
            icrc_wasm_module,
            encode_one(icrc1_deploy_args).unwrap(),
            Some(caller),
        );

        // Create an intent to transfer ICRC1 tokens
        let transfer_amount = 100_000_000_000.0;
        let proposed_tx = ProposeTransactionArgs {
            transaction_type: TransactionType::Transfer,
            amount: transfer_amount,
            network: SupportedNetwork::ICP,
            to: receiver.to_text(),
            token: format!("icp:icrc1:{}", icrc_ledger.to_text()),
        };

        // Add the intent
        let add_intent_result: (ProposedTransaction,) = update_candid_as(
            &pic,
            account_id,
            caller,
            "propose_transaction",
            (proposed_tx,),
        )
        .unwrap();

        // Execute the intent
        let status: (IntentStatus,) = update_candid_as(
            &pic,
            account_id,
            caller,
            "execute_transaction",
            (add_intent_result.0.id,),
        )
        .unwrap();

        assert_eq!(
            status.0,
            IntentStatus::Completed("Successfully transferred an ICRC-1 token.".to_string())
        );

        // Check the receiver's balance
        let receiver_balance: (u128,) = query_candid_as(
            &pic,
            icrc_ledger,
            caller,
            "icrc1_balance_of",
            (ICRCAccount::new(receiver, None),),
        )
        .unwrap();

        assert_eq!(receiver_balance.0, transfer_amount as u128);

        // Check the account canister's balance
        let account_balance: (u128,) = query_candid_as(
            &pic,
            icrc_ledger,
            caller,
            "icrc1_balance_of",
            (ICRCAccount::new(account_id, None),),
        )
        .unwrap();

        assert_eq!(
            account_balance.0,
            mint_amount_u64 as u128 - transfer_amount as u128 - 1_000_000 as u128 // Subtract transfer amount and fee
        );
    }

    #[test]
    fn should_transfer_icp() {
        let pic = PocketIcBuilder::new()
            .with_application_subnet()
            .with_nns_subnet()
            .with_ii_subnet()
            .build();
        let caller = generate_principal();

        // Create and set up the account canister
        let account_id = pic.create_canister_with_settings(Some(caller), None);
        pic.add_cycles(account_id, 2_000_000_000_000);
        let wasm_module =
            include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();
        pic.install_canister(account_id, wasm_module, Vec::new(), Some(caller));

        // Create a receiver principal
        let receiver = generate_principal();

        // Set up the ICP ledger canister
        let specified_nns_ledger_canister_id =
            Principal::from_text("ryjl3-tyaaa-aaaaa-aaaba-cai").unwrap();
        let nns_ledger_canister_id = pic
            .create_canister_with_id(Some(caller), None, specified_nns_ledger_canister_id)
            .unwrap();
        assert_eq!(nns_ledger_canister_id, specified_nns_ledger_canister_id);

        let icp_ledger_canister_wasm: Vec<u8> = include_bytes!("icp-ledger.wasm.gz").to_vec(); // get the ICP ledger wasm
        let minter = generate_principal(); // some principal not used anywhere else
        let minting_account = AccountIdentifier::new(&minter, &DEFAULT_SUBACCOUNT);
        let icp_ledger_init_args = LedgerCanisterPayload::Init(NnsLedgerCanisterInitPayload {
            minting_account: minting_account.to_string(),
            icrc1_minting_account: None,
            initial_values: vec![
                (
                    minting_account.to_string(),
                    Tokens::from_e8s(100_000_000_000_000),
                ),
                (
                    AccountIdentifier::new(&caller, &DEFAULT_SUBACCOUNT).to_string(),
                    Tokens::from_e8s(100_000_000_000_000),
                ),
                (
                    AccountIdentifier::new(&account_id, &DEFAULT_SUBACCOUNT).to_string(),
                    Tokens::from_e8s(100_000_000_000_000),
                ),
            ], // fill in some initial account balances
            max_message_size_bytes: None,
            transaction_window: None,
            archive_options: None,
            send_whitelist: HashSet::new(),
            transfer_fee: Some(Tokens::from_e8s(10_000)),
            token_symbol: Some("ICP".to_string()),
            token_name: Some("Internet Computer".to_string()),
            feature_flags: None,
            maximum_number_of_accounts: None,
            accounts_overflow_trim_quantity: None,
        });

        pic.install_canister(
            nns_ledger_canister_id,
            icp_ledger_canister_wasm,
            encode_one(icp_ledger_init_args).unwrap(),
            Some(caller),
        );

        // Create an intent to transfer ICP
        let transfer_amount = 100_000_000.0; // 1 ICP
        let receiver_account = AccountIdentifier::new(&receiver, &DEFAULT_SUBACCOUNT);
        let proposed_tx: ProposeTransactionArgs = ProposeTransactionArgs {
            transaction_type: TransactionType::Transfer,
            amount: transfer_amount,
            network: SupportedNetwork::ICP,
            to: format!("{}", receiver_account.to_string()),
            token: "icp:native".to_string(),
        };

        // Add the intent
        let add_intent_result: (ProposedTransaction,) = update_candid_as(
            &pic,
            account_id,
            caller,
            "propose_transaction",
            (proposed_tx,),
        )
        .unwrap();

        // Execute the intent
        let execute_result = pic.update_call(
            account_id,
            caller,
            "execute_transaction",
            encode_one(add_intent_result.0.id).unwrap(),
        );
        let status = match execute_result {
            Ok(WasmResult::Reply(reply)) => Decode!(&reply, IntentStatus).unwrap(),
            Ok(WasmResult::Reject(reject_message)) => {
                panic!("Execute intent call rejected: {}", reject_message)
            }
            Err(err) => panic!("Execute intent call failed: {:?}", err),
        };

        assert_eq!(
            status,
            IntentStatus::Completed("Successfully transferred native ICP.".to_string())
        );

        // Check the receiver's balance
        let receiver_balance_args = AccountBalanceArgs {
            account: AccountIdentifier::new(&receiver, &DEFAULT_SUBACCOUNT),
        };
        let receiver_balance_result: (Tokens,) = query_candid_as(
            &pic,
            specified_nns_ledger_canister_id,
            caller,
            "account_balance",
            (receiver_balance_args,),
        )
        .unwrap();
        let receiver_balance = receiver_balance_result.0;

        assert_eq!(receiver_balance.e8s(), transfer_amount as u64);

        // Check the account canister's balance
        let account_balance_args = AccountBalanceArgs {
            account: AccountIdentifier::new(&account_id, &DEFAULT_SUBACCOUNT),
        };
        let account_balance_result: (Tokens,) = query_candid_as(
            &pic,
            specified_nns_ledger_canister_id,
            caller,
            "account_balance",
            (account_balance_args,),
        )
        .unwrap();
        let account_balance = account_balance_result.0;

        assert_eq!(
            account_balance.e8s(),
            100_000_000_000_000 - transfer_amount as u64 - RECOMMENDED_ICP_TRANSACTION_FEE
        );
    }
}
