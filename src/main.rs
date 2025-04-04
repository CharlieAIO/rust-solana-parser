mod tx_parser;
mod swap_parser;

use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta,
    UiTransactionEncoding,
};
use std::fmt::Debug;
use std::str::FromStr;
use crate::tx_parser::TokenBalanceDiff;

const RAYDIUM_V4:&str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const RPC_URL: &str = "https://api.mainnet-beta.solana.com";



struct Transfer {
    token_balance_diff: TokenBalanceDiff,
    to_user_account: Pubkey,
    to_token_account: Pubkey,
    from_user_account: Pubkey,
    from_token_account: Pubkey,
}

fn fetch_tx(signature: &Signature) -> EncodedConfirmedTransactionWithStatusMeta {
    let rpc_client = RpcClient::new(RPC_URL.to_string());
    let tx = rpc_client.get_transaction_with_config(
        signature,
        RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::Base64),
            commitment: None,
            max_supported_transaction_version: Some(2),
        },
    );

    tx.unwrap_or_else(|e| panic!("Error: {:?}", e))
}


fn main() {
    let transaction_signarure = Signature::from_str(
        "2qLvZ13vVxbyBP6JBezdps2uymqSKeFTXE7JbktLD97sVZXrCdaDTReLraNv5PXv9h6q83KZK1tzHKkroDwucvhH",
    )
    .unwrap();
    let tx = fetch_tx(&transaction_signarure);
    let meta = tx.transaction.meta.clone().unwrap();
    let transaction = match tx.transaction.transaction.clone().decode() {
        Some(t) => t,
        None => {
            println!("Error: Transaction failed to decode");
            return;
        }
    };

    let balance_map = tx_parser::create_balance_diff_map(&meta);
    let native_balance_map = tx_parser::create_native_balance_diff_map(&meta);
    let account_keys = tx_parser::get_all_account_keys(&meta, transaction.message.static_account_keys());



    let token_account_map =
        tx_parser::build_token_account_map(transaction.message.instructions(), &meta, &account_keys);

    let transfers = tx_parser::parse_instructions(
        transaction.message.instructions(),
        &meta.inner_instructions,
        &balance_map,
        &native_balance_map,
        &account_keys,
        &token_account_map,
    );
    // println!("Transfers: {:?}", transfers);
    let swaps = swap_parser::parse_swaps(transfers, &account_keys);
    println!("Swaps: {:?}", swaps);
}
