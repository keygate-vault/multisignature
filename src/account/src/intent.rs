use std::{collections::HashMap, future::Future, pin::Pin};

use crate::evm;
use candid::{CandidType, Nat, Principal};
use dyn_clone::DynClone;
use ic_cdk::api::call::CallResult;
use ic_ledger_types::{
    AccountIdentifier, BlockIndex, Memo, Tokens, TransferArgs, MAINNET_LEDGER_CANISTER_ID,
};
use icrc_ledger_types::icrc1::{
    account::Account,
    transfer::{TransferArg as ICRC1TransferArgs, TransferError},
};
use serde_bytes::ByteBuf;

use crate::{
    evm_types::TransactionRequestBasic, get_default_icrc_subaccount, to_subaccount, ADAPTERS,
    PROPOSED_TRANSACTIONS, THRESHOLD,
};

use std::{
    borrow::Cow,
    fmt::{self, Display},
};

use ic_cdk::{query, update};
use ic_stable_structures::{storable::Bound, Storable};
use serde::{Deserialize, Serialize};

use crate::TRANSACTIONS;

pub(crate) trait BlockchainAdapter: DynClone {
    fn network(&self) -> SupportedNetwork;
    fn token(&self) -> String;
    fn intent_type(&self) -> TransactionType;
    fn execute<'a>(
        &'a self,
        transaction: &'a TransactionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<IntentStatus, String>> + 'a>>;
}

dyn_clone::clone_trait_object!(BlockchainAdapter);

pub type TokenPath = String;

pub async fn execute(transaction: &TransactionRequest) -> IntentStatus {
    let it: &'static str = transaction.transaction_type.clone().into();

    let token = transaction.token.clone();

    // if icrc, ignore last token : split of str
    let token_parts: Vec<&str> = token.split(':').collect();
    ic_cdk::println!("Token parts: {:?}", token_parts);

    let token_key = if token_parts.len() > 2 && token_parts[1] == "icrc1" {
        // For ICRC, ignore the last part
        token_parts[..token_parts.len() - 1].join(":") + ":" + it.to_ascii_lowercase().as_str()
    } else {
        token.to_string() + ":" + it.to_ascii_lowercase().as_str()
    };

    ic_cdk::println!("Token key: {:?}", token_key);

    ic_cdk::println!("Searching for adapter...");
    let adapter = ADAPTERS.with(
        |adapters: &std::cell::RefCell<HashMap<String, Box<dyn BlockchainAdapter>>>| {
            dyn_clone::clone_box(
                adapters
                    .borrow()
                    .get(&token_key)
                    .expect(&format!("Adapter not found for {}", token_key)),
            )
        },
    );

    ic_cdk::println!("Adapter found.");

    match adapter.execute(transaction).await {
        Ok(status) => status,
        Err(e) => {
            ic_cdk::println!("Error executing intent: {}", e);
            IntentStatus::Failed(e)
        }
    }
}

#[derive(Clone)]
pub struct ICPNativeTransferAdapter {
    pub(crate) network: SupportedNetwork,
    pub(crate) token: TokenPath,
    pub(crate) intent_type: TransactionType,
}

type ICPNativeTransferArgs = TransferArgs;

/**
 * See TransferArgs in ic_ledger_types
 */
pub const RECOMMENDED_ICP_TRANSACTION_FEE: u64 = 10000;
pub const RECOMMENDED_ICRC1_TRANSACTION_FEE: u64 = 1000000;

impl BlockchainAdapter for ICPNativeTransferAdapter {
    fn network(&self) -> SupportedNetwork {
        self.network.clone()
    }

    fn token(&self) -> TokenPath {
        self.token.clone()
    }

    fn intent_type(&self) -> TransactionType {
        self.intent_type.clone()
    }

    fn execute<'a>(
        &'a self,
        transaction: &'a TransactionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<IntentStatus, String>> + 'a>> {
        Box::pin(async move {
            ic_cdk::println!("Executing ICPAdapter");

            ic_cdk::println!("Fee: {:?}", RECOMMENDED_ICP_TRANSACTION_FEE);

            let args = ICPNativeTransferArgs {
                to: AccountIdentifier::from_hex(&transaction.to).unwrap(),
                amount: Tokens::from_e8s(transaction.amount as u64),
                fee: Tokens::from_e8s(RECOMMENDED_ICP_TRANSACTION_FEE),
                memo: Memo(0),
                from_subaccount: Some(to_subaccount(0)),
                created_at_time: None,
            };

            ic_cdk::println!("Args: {:?}", args);

            match ICPNativeTransferAdapter::transfer(args).await {
                Ok(_) => Ok(IntentStatus::Completed(
                    "Successfully transferred native ICP.".to_string(),
                )),
                Err(e) => Err(e.to_string()),
            }
        })
    }
}

impl ICPNativeTransferAdapter {
    pub fn new() -> ICPNativeTransferAdapter {
        ICPNativeTransferAdapter {
            network: SupportedNetwork::ICP,
            token: "icp:native".to_string(),
            intent_type: TransactionType::Transfer,
        }
    }

    async fn transfer(args: ICPNativeTransferArgs) -> Result<BlockIndex, String> {
        match ic_ledger_types::transfer(MAINNET_LEDGER_CANISTER_ID, args).await {
            Ok(Ok(block_index)) => return Ok(block_index),
            Ok(Err(transfer_error)) => {
                let error_message = format!("transfer error: {:?}", transfer_error);
                Err(error_message)
            }
            Err((error, message)) => {
                let error_message = format!("unexpected error: {:?}, message: {}", error, message);
                Err(error_message)
            }
        }
    }
}

#[derive(Clone)]
pub struct ETHNativeTransferAdapter {
    pub(crate) network: SupportedNetwork,
    pub(crate) token: TokenPath,
    pub(crate) intent_type: TransactionType,
}

impl ETHNativeTransferAdapter {
    pub fn new() -> ETHNativeTransferAdapter {
        ETHNativeTransferAdapter {
            network: SupportedNetwork::ETH,
            token: "eth:native".to_string(),
            intent_type: TransactionType::Transfer,
        }
    }

    async fn transfer(&self, transaction: &TransactionRequest) -> Result<String, String> {
        ic_cdk::println!("Executing ETHAdapter");
        let request = TransactionRequestBasic {
            to: transaction.to.clone(),
            value: transaction.amount.to_string(),
            chain: "eth".to_string(),
        };
        let result = evm::execute_transaction_evm(request).await;
        match result.status.as_str() {
            "Success" => Ok(result.hash),
            _ => Err(result.status),
        }
    }
}

impl BlockchainAdapter for ETHNativeTransferAdapter {
    fn network(&self) -> SupportedNetwork {
        self.network.clone()
    }

    fn token(&self) -> String {
        self.token.clone()
    }

    fn intent_type(&self) -> TransactionType {
        self.intent_type.clone()
    }

    fn execute<'a>(
        &'a self,
        transaction: &'a TransactionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<IntentStatus, String>> + 'a>> {
        Box::pin(async move {
            ic_cdk::println!("Executing ETHAdapter");
            match self.transfer(transaction).await {
                Ok(result) => Ok(IntentStatus::Completed(
                    "Successfully transferred native ETH: ".to_string() + &result,
                )),
                Err(e) => Err(e.to_string()),
            }
        })
    }
}

#[derive(Clone)]
pub struct ICRC1TransferAdapter {
    pub(crate) network: SupportedNetwork,
    pub(crate) token: TokenPath,
    pub(crate) intent_type: TransactionType,
}

impl BlockchainAdapter for ICRC1TransferAdapter {
    fn network(&self) -> SupportedNetwork {
        self.network.clone()
    }

    fn token(&self) -> String {
        self.token.clone()
    }

    fn intent_type(&self) -> TransactionType {
        self.intent_type.clone()
    }

    fn execute<'a>(
        &'a self,
        transaction: &'a TransactionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<IntentStatus, String>> + 'a>> {
        Box::pin(async move {
            ic_cdk::println!("Executing ICRC1Adapter");
            match self.transfer(transaction).await {
                // TODO: include the name or symbol of the token
                Ok(_) => Ok(IntentStatus::Completed(
                    "Successfully transferred an ICRC-1 token.".to_string(),
                )),
                Err(e) => Err(e.to_string()),
            }
        })
    }
}

impl ICRC1TransferAdapter {
    pub fn new() -> ICRC1TransferAdapter {
        ICRC1TransferAdapter {
            network: SupportedNetwork::ICP,
            token: "icp:icrc1".to_string(),
            intent_type: TransactionType::Transfer,
        }
    }

    pub fn extract_token_identifier(token: TokenPath) -> Result<String, String> {
        let parts: Vec<&str> = token.split(':').collect();
        if parts.len() != 3 {
            return Err("Invalid token format".to_string());
        }
        Ok(parts[2].to_string())
    }

    async fn transfer(&self, transaction: &TransactionRequest) -> Result<Nat, String> {
        ic_cdk::println!("Executing ICRC1TransferAdapter");

        let args = ICRC1TransferArgs {
            to: Account {
                owner: Principal::from_text(&transaction.to).unwrap(),
                subaccount: None,
            },
            amount: Nat::from(transaction.amount as u64),
            fee: Some(Nat::from(RECOMMENDED_ICRC1_TRANSACTION_FEE)),
            memo: Some(icrc_ledger_types::icrc1::transfer::Memo(ByteBuf::from(
                vec![],
            ))),
            from_subaccount: Some(get_default_icrc_subaccount().0),
            created_at_time: None,
        };

        ic_cdk::println!("Args: {:?}", args);

        let token_identifier =
            ICRC1TransferAdapter::extract_token_identifier(transaction.token.clone())?;
        let principal = Principal::from_text(&token_identifier).unwrap();

        let transfer_result: CallResult<(Result<Nat, TransferError>,)> =
            ic_cdk::call(principal, "icrc1_transfer", (args,)).await;
        match transfer_result {
            Ok((inner_result,)) => match inner_result {
                Ok(block_index) => Ok(block_index),
                Err(transfer_error) => Err(format!("ICRC-1 transfer error: {:?}", transfer_error)),
            },
            Err((rejection_code, message)) => Err(format!(
                "Canister call rejected: {:?} - {}",
                rejection_code, message
            )),
        }
    }
}

#[derive(
    CandidType, Deserialize, Serialize, Debug, Clone, PartialEq, strum_macros::IntoStaticStr,
)]
pub enum TransactionType {
    Swap,
    Transfer,
}

#[derive(
    CandidType, Deserialize, Serialize, Debug, Clone, PartialEq, strum_macros::IntoStaticStr,
)]
pub enum IntentStatus {
    Pending(String),
    InProgress(String),
    Completed(String),
    Rejected(String),
    Failed(String),
}

#[derive(
    CandidType, Deserialize, Serialize, Debug, Clone, PartialEq, strum_macros::IntoStaticStr,
)]
pub enum SupportedNetwork {
    ICP,
    ETH,
}

// Formats for tokens:
// icp:native
// icp:icrc1:<principal_id>
// eth:{erc20}:{0x0000000000000000000000000000000000000000}
#[derive(CandidType, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Token(pub String);

impl Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents an intent for a blockchain transaction.
///
/// An `Intent` encapsulates all the necessary information for executing
/// a transaction on a supported blockchain network.
///
/// # Fields
///
/// * `intent_type` - The type of the intent (e.g., transfer, swap).
/// * `amount` - The amount of tokens involved in the transaction.
/// * `token` - The token identifier for the transaction. For native ICP, it's "ICP:native". For ICRC-1 tokens, it's "ICP:<icrc_standard>:<principal_id>". For ETH, it's "eth:<token_standard>:<token_address>".
/// * `to` - The recipient's address or identifier. For ICP and ICRC-1 tokens, it's in the format <principal_id>.<subaccount_id>, where subaccount_id is Base32 encoded. For ETH, it's the address of the recipient.
/// * `network` - The blockchain network on which the transaction should occur.
/// * `status` - The current status of the intent.
///
/// # Examples
///
/// ```
/// use intent::{Intent, IntentType, Token, SupportedNetwork, IntentStatus};
/// use ic_cdk::export::Principal;
///
/// let principal = Principal::from_text("l4sux-ovcbh-qjlir-sij5b-xffaa-uydtf-zlwgz-ezskj-zlar3-4ap3v-2qe").unwrap();
/// let subaccount = [0u8; 32];  // Example subaccount
/// let to_address = account_to_string(&principal, Some(&subaccount));
///
/// let intent = Intent {
///     intent_type: IntentType::Transfer,
///     amount: 1000000,
///     token: Token("icp:icrc1:z3hc7-f3wle-sfb34-ftgza-o7idl-vopan-733dp-5s6vi-wy4zo-tzwmv-4ae".to_string()),
///     to: to_address,
///     network: SupportedNetwork::ICP,
///     status: IntentStatus::Pending,
/// };
/// ```
#[derive(CandidType, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Intent {
    pub transaction_type: TransactionType,
    pub amount: f64,
    pub token: TokenPath,
    pub to: String,
    pub network: SupportedNetwork,
    pub status: IntentStatus,
}

#[derive(CandidType, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct TransactionRequest {
    pub transaction_type: TransactionType,
    pub amount: f64,
    pub token: TokenPath,
    pub to: String,
    pub network: SupportedNetwork,
}

#[derive(CandidType, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Transaction {
    pub status: IntentStatus,
    pub to: String,
    pub token: TokenPath,
    pub network: SupportedNetwork,
    pub amount: f64,
    pub transaction_type: TransactionType,
}

impl Storable for Transaction {
    const BOUND: Bound = Bound::Bounded {
        max_size: 1024,
        is_fixed_size: false,
    };

    fn to_bytes(&self) -> Cow<[u8]> {
        let serialized = serde_cbor::to_vec(self).expect("Serialization failed");
        Cow::Owned(serialized)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let deserialized: Transaction = serde_cbor::from_slice(&bytes.to_vec()).unwrap();
        deserialized
    }
}

#[derive(CandidType, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ProposedTransaction {
    pub id: u64,
    pub to: String,
    pub token: TokenPath,
    pub network: SupportedNetwork,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub signers: Vec<Principal>,
    pub rejections: Vec<Principal>,
}

impl Storable for ProposedTransaction {
    const BOUND: Bound = Bound::Bounded {
        max_size: 1024,
        is_fixed_size: false,
    };

    fn to_bytes(&self) -> Cow<[u8]> {
        let serialized = serde_cbor::to_vec(self).expect("Serialization failed");
        Cow::Owned(serialized)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let deserialized: ProposedTransaction = serde_cbor::from_slice(&bytes.to_vec()).unwrap();
        deserialized
    }
}

impl Intent {
    pub fn network(&self) -> SupportedNetwork {
        self.network.clone()
    }

    pub fn token(&self) -> TokenPath {
        self.token.clone()
    }

    pub fn intent_type(&self) -> TransactionType {
        self.transaction_type.clone()
    }

    pub fn status(&self) -> IntentStatus {
        self.status.clone()
    }

    pub fn to(&self) -> String {
        self.to.clone()
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }
}

impl Storable for Intent {
    const BOUND: Bound = Bound::Bounded {
        max_size: 1024,
        is_fixed_size: false,
    };

    fn to_bytes(&self) -> Cow<[u8]> {
        let serialized = serde_cbor::to_vec(self).expect("Serialization failed");
        Cow::Owned(serialized)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let deserialized: Intent = serde_cbor::from_slice(&bytes.to_vec()).unwrap();
        deserialized
    }
}

#[derive(CandidType, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Decision {
    id: u64,
    intent_id: u64,
    signee: Principal,
    approved: bool,
}

impl Decision {
    pub fn new(id: u64, intent_id: u64, signee: Principal, approved: bool) -> Decision {
        Decision {
            id,
            intent_id,
            signee,
            approved,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn approved(&self) -> bool {
        self.approved
    }
}

#[ic_cdk::query]
pub fn get_adapters() -> Vec<String> {
    ADAPTERS.with(|adapters| adapters.borrow().keys().cloned().collect())
}

#[ic_cdk::query]
pub fn get_transactions() -> Vec<Transaction> {
    TRANSACTIONS.with(|transactions| {
        let transactions = transactions.borrow();
        transactions.iter().collect()
    })
}

#[update]
pub async fn execute_transaction(proposal_id: u64) -> IntentStatus {
    let proposal = PROPOSED_TRANSACTIONS
        .with(|proposed_transactions| proposed_transactions.borrow().get(proposal_id));

    if proposal.is_none() {
        return IntentStatus::Failed(format!("Proposal not found: {}", proposal_id));
    }

    // check if threshold is met
    let threshold = THRESHOLD.with(|threshold| threshold.borrow().get().clone());

    let signers = proposal.clone().unwrap().signers;
    let length = signers.len() as u64;
    let proposal = proposal.unwrap();

    if length < threshold {
        return IntentStatus::Failed("Threshold not met".to_string());
    }

    let transaction = TransactionRequest {
        transaction_type: proposal.transaction_type,
        amount: proposal.amount,
        token: proposal.token,
        to: proposal.to,
        network: proposal.network,
    };

    let execution_result = super::execute(&transaction).await;

    ic_cdk::println!("Executing transaction: {:?}", transaction);

    ic_cdk::println!("Execution result: {:?}", execution_result);

    TRANSACTIONS.with(|transactions| {
        let transactions = transactions.borrow_mut();
        let transaction = Transaction {
            status: execution_result.clone(),
            to: transaction.to,
            token: transaction.token,
            network: transaction.network,
            amount: transaction.amount,
            transaction_type: transaction.transaction_type,
        };

        println!("Appending transaction: {:?}", transaction);
        match transactions.append(&transaction) {
            Ok(_) => (),
            Err(e) => panic!("Failed to append transaction: {:?}", e),
        }
    });

    execution_result
}
