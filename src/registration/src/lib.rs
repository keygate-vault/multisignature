use std::collections::HashMap;
use std::cell::RefCell;

use ic_cdk::{query, update};
use candid::{CandidType, Deserialize, Principal};

#[derive(Clone, CandidType, Deserialize)]
struct UserInfo {
    first_name: String,
    last_name: String,
}

thread_local! {
    static USERS: RefCell<HashMap<Principal, UserInfo>> = RefCell::default();
}

#[update]
fn register_user(principal: Principal, first_name: String, last_name: String) {
    USERS.with(|users| {
        let mut users = users.borrow_mut();
        if users.contains_key(&principal) {
            ic_cdk::trap(&format!("User with principal {} already exists", principal));
        }
        users.insert(principal, UserInfo { first_name, last_name });
    });
}

#[query]
fn get_user(principal: Principal) -> Option<UserInfo> {
    USERS.with(|users| {
        users.borrow().get(&principal).cloned()
    })
}

#[query]
fn user_exists(principal: Principal) -> bool {
    USERS.with(|users| {
        users.borrow().contains_key(&principal)
    })
}

ic_cdk::export_candid!();