mod deployer;
mod types;

#[cfg(test)]
mod tests;

use std::{cell::RefCell};

use ic_cdk::{init, query, update};
use candid::Principal;
use ic_stable_structures::{memory_manager::{MemoryId, MemoryManager}, StableBTreeMap, DefaultMemoryImpl};
use types::{Memory, UserInfo};

const USERS_MEMORY: MemoryId = MemoryId::new(0);
const VAULTS_MEMORY: MemoryId = MemoryId::new(1);

thread_local! {
    static WALLET_WASM: RefCell<Option<Vec<u8>>> = RefCell::default();

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    pub static STABLE_USERS: RefCell<StableBTreeMap<Principal, UserInfo, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(USERS_MEMORY))
        )
    );

    // Map from vault canister id to owner principal.
    pub static STABLE_VAULTS: RefCell<StableBTreeMap<Principal, Principal, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(VAULTS_MEMORY))
        )
    );
}

#[query]
fn get_wasm() -> Vec<u8> {
    WALLET_WASM.with(|wasm| wasm.borrow().clone().unwrap())
}

#[init]
fn init() {
    load_wallet_wasm();
}

#[update]
fn register_user() {
    let principal = ic_cdk::caller();

    STABLE_USERS.with(|users| {
        let mut users = users.borrow_mut();
        if users.contains_key(&principal) {
            ic_cdk::trap(&format!("User with principal {} already exists", principal));
        }
        users.insert(principal, UserInfo { vaults: vec![] });
    });
}

#[update]
async fn upgrade_account(canister_id: Principal) -> Result<(), String> {
    let owner_principal = ic_cdk::caller();

    let canister_owner = STABLE_VAULTS.with(|vaults| {
        vaults.borrow().get(&canister_id)
    });

    if canister_owner != Some(owner_principal) {
        return Err(format!("Only the owner of the vault canister can upgrade it"));
    }

    load_wallet_wasm();

    match deployer::upgrade(canister_id, WALLET_WASM.with(|wasm| wasm.borrow().clone().unwrap())).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to upgrade account: {}", e)),
    }
}

fn user_exists(principal: Principal) -> bool {
    STABLE_USERS.with(|users| users.borrow().contains_key(&principal))
}

/**
 * TODO: Add vault name to init args of the vault canister.
 */
#[update]
async fn deploy_account() -> Principal {
    // Verify caller is registered
    let owner_principal = ic_cdk::caller();
    if !user_exists(owner_principal) {
        ic_cdk::trap(&format!("User with principal {} not found", owner_principal));
    }

    let wallet_wasm = WALLET_WASM.with(|wasm| {
        wasm.borrow().clone().unwrap_or_else(|| ic_cdk::trap("Wallet wasm not loaded"))
    });

    match deployer::deploy(wallet_wasm).await {
        Ok(canister_id) => {
            // Add to ownership hash map
            STABLE_VAULTS.with(|vaults| {
                let mut vaults = vaults.borrow_mut();
                vaults.insert(canister_id, owner_principal);
            });

            // Add vault to user
            STABLE_USERS.with(|users| {
                let mut users = users.borrow_mut();
                let user = users.get(&owner_principal);

                match user {
                    Some(user) => {
                        let mut user = user.clone();
                        user.vaults.push(canister_id);
                        users.insert(owner_principal, user);
                    },
                    None => ic_cdk::trap(&format!("User with principal {} not found", owner_principal)),
                }
            });

            canister_id
        }
        Err(err) => ic_cdk::trap(&format!("Failed to deploy account: {}", err)),
    }
}

fn load_wallet_wasm() {
    let wasm_module: Vec<u8> = include_bytes!("../../../target/wasm32-unknown-unknown/release/account.wasm").to_vec();
    WALLET_WASM.with(|wasm| {
        *wasm.borrow_mut() = Some(wasm_module);
    });
}

#[update]
fn load_wallet_wasm_blob(wasm_blob: Vec<u8>) {
    WALLET_WASM.with(|wasm| {
        *wasm.borrow_mut() = Some(wasm_blob);
    });
}

#[query]
fn get_user() -> Option<UserInfo> {
    let principal = ic_cdk::caller(); 

    if !user_exists(principal) {
        ic_cdk::trap(&format!("User with principal {} not found", principal));
    }

    STABLE_USERS.with(|users| users.borrow().get(&principal))
}

#[query]
fn get_user_vaults() -> Vec<Principal> {
    let owner_principal = ic_cdk::caller();

    if !user_exists(owner_principal) {
        ic_cdk::trap(&format!("User with principal {} not found", owner_principal));
    }

    let mut user_vaults = vec![];

    STABLE_USERS.with(|users| {
        let users = users.borrow();
        let user = users.get(&owner_principal);

        match user {
            Some(user) => user_vaults = user.vaults.clone(),
            None => user_vaults = vec![],
        }
    });

    user_vaults
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    load_wallet_wasm();
}

ic_cdk::export_candid!();