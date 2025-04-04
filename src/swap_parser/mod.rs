use std::collections::HashMap;
use std::str::FromStr;
use std::thread::current;
use solana_sdk::pubkey::Pubkey;
use crate::RAYDIUM_V4;
use crate::tx_parser::Transfer;

#[derive(Debug)]
pub struct Swap {
    wallet: String,
    token_in: String,
    token_out: String,
    amount_in: f64,
    amount_out: f64,
}

pub fn parse_swaps(
    transfers: Vec<Transfer>,
    account_keys: &HashMap<u8, Pubkey>,
) -> Vec<Swap> {
    let mut swaps = Vec::<Swap>::new();

    for (index,transfer) in transfers.iter().enumerate() {
        if let Some(swap) = process_transfer(index, &transfers, account_keys) {
            if transfer.instruction_program_id.is_none() {
                continue;
            }
            swaps.push(swap);
        }
    }

    swaps
}

pub fn process_transfer(
    index: usize,
    transfers: &Vec<Transfer>,
    account_keys: &HashMap<u8, Pubkey>,
) -> Option<Swap> {
    if index + 1 >= transfers.len() {
        return None;
    }
    let transfer = &transfers[index];
    let next_transfer = &transfers[index + 1];

    if transfer.instruction_program_id != Option::from(Pubkey::from_str(RAYDIUM_V4).unwrap()) {
        return None;
    }

    let mut wallet = account_keys
        .get(transfer.instruction_input_accounts.as_ref()?.get(16)?)
        .unwrap()
        .to_string();

    if let Some(input_accounts) = transfer.instruction_input_accounts.as_ref() {
        if input_accounts.len() == 18 {
            if let Some(account_key) = input_accounts.get(17) {
                if let Some(account) = account_keys.get(account_key) {
                    wallet = account.to_string();
                }
            }
        }
    }

    let token_in = next_transfer.token_balance_diff.mint.clone();
    let token_out = transfer.token_balance_diff.mint.clone();

    Some(Swap{
        wallet,
        token_in: token_in.to_string(),
        token_out: token_out.to_string(),
        amount_in: next_transfer.token_balance_diff.token_amount,
        amount_out: transfer.token_balance_diff.token_amount,
    })
}