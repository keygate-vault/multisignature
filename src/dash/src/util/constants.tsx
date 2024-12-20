import { SubAccount } from "@dfinity/ledger-icp";

export const ICP_DECIMALS = 8;

export const DEFAULT_SUBACCOUNT = SubAccount.fromBytes(
  Uint8Array.from([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
  ])
) as SubAccount;

if (!process.env.CANISTER_ID_ICRC1_LEDGER_CANISTER) {
  throw new Error("CANISTER_ID_ICRC1_LEDGER_CANISTER is not set");
}

export const NATIVE_ICP_CANISTER = "ryjl3-tyaaa-aaaaa-aaaba-cai";
export const MOCK_ICRC1_CANISTER = "bd3sg-teaaa-aaaaa-qaaba-cai";
export const CKETH_CANISTER_ID = "ss2fx-dyaaa-aaaar-qacoq-cai";
export const CKBTC_CANISTER_ID = "mxzaz-hqaaa-aaaar-qaada-cai";
export const CKUSDC_CANISTER_ID = "xevnm-gaaaa-aaaar-qafnq-cai";

export const TOKEN_URN_TO_SYMBOL = {
  "icp:native": "ICP",
  [`icp:icrc1:${process.env.CANISTER_ID_ICRC1_LEDGER_CANISTER}`]: "MCK",
  "eth:native": "ETH",
  [`icp:icrc1:${CKETH_CANISTER_ID}`]: "ckETH",
  [`icp:icrc1:${CKBTC_CANISTER_ID}`]: "ckBTC",
  [`icp:icrc1:${CKUSDC_CANISTER_ID}`]: "ckUSDC",
};
