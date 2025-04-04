use solana_sdk::bs58;
use solana_sdk::instruction::CompiledInstruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::system_instruction::SystemInstruction;
use solana_transaction_status::option_serializer::OptionSerializer;
use solana_transaction_status::{
    UiInnerInstructions, UiInstruction, UiTransactionStatusMeta,
};
use spl_token::instruction::TokenInstruction;
use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;
use std::string::ToString;
use crate::RAYDIUM_V4;

#[derive(Debug, Clone)]
pub struct TokenBalanceDiff {
    pub token_amount: f64,
    pub mint: String,
    pub decimals: u8,
}

#[derive(Debug)]
pub struct Transfer {
    pub instruction_program_id: Option<Pubkey>,
    pub instruction_input_accounts: Option<Vec<u8>>,
    pub token_balance_diff: TokenBalanceDiff,
    pub to_user_account: Pubkey,
    pub to_token_account: Pubkey,
    pub from_user_account: Pubkey,
    pub from_token_account: Pubkey,
}
pub fn create_balance_diff_map(meta: &UiTransactionStatusMeta) -> HashMap<u8, TokenBalanceDiff> {
    let mut token_balance_diff_map: HashMap<u8, TokenBalanceDiff> = HashMap::new();

    let post_token_balances = meta.post_token_balances.clone();
    let pre_token_balances = meta.pre_token_balances.clone();

    for post in post_token_balances.unwrap().iter() {
        let token_account = post.account_index;
        let token_amount = post.ui_token_amount.ui_amount.unwrap_or(0.0);
        token_balance_diff_map.insert(
            token_account,
            TokenBalanceDiff {
                token_amount,
                mint: post.mint.clone(),
                decimals: post.ui_token_amount.decimals,
            },
        );
    }

    for pre in pre_token_balances.unwrap().iter() {
        if token_balance_diff_map.get(&pre.account_index).is_none() {
            continue;
        }
        let token_account = pre.account_index;
        let token_amount = pre.ui_token_amount.ui_amount.unwrap_or(0.0);

        let token_balance_map_entry = token_balance_diff_map.get_mut(&token_account).unwrap();

        let diff = token_balance_map_entry.token_amount - token_amount;
        token_balance_map_entry.token_amount = diff;
    }

    token_balance_diff_map
}

pub fn create_native_balance_diff_map(meta: &UiTransactionStatusMeta) -> HashMap<u8, f64> {
    let mut native_balance_diff_map: HashMap<u8, f64> = HashMap::new();

    let post_balances = meta.post_balances.clone();
    let pre_balances = meta.pre_balances.clone();

    for (account_index, post_amount) in post_balances.iter().enumerate() {
        native_balance_diff_map.insert(account_index as u8, *post_amount as f64 / 1_000_000_000.0);
    }

    for (account_index, pre_amount) in pre_balances.iter().enumerate() {
        if let Some(balance_map_entry) = native_balance_diff_map.get_mut(&(account_index as u8)) {
            let pre_amount_sol = *pre_amount as f64 / 1_000_000_000.0;
            *balance_map_entry = (*balance_map_entry - pre_amount_sol).max(0.0);
        }
    }

    native_balance_diff_map
}

pub fn get_all_account_keys(
    meta: &UiTransactionStatusMeta,
    account_keys: &[Pubkey],
) -> HashMap<u8, Pubkey> {
    let mut account_keys_map: HashMap<u8, Pubkey> = HashMap::new();
    let mut index = 0;
    for account in account_keys.iter() {
        account_keys_map.insert(index, *account);
        index += 1;
    }

    let loaded_addresses = &meta.loaded_addresses.clone().unwrap();

    for writable in loaded_addresses.writable.iter() {
        account_keys_map.insert(index, writable.parse().unwrap());
        index += 1;
    }
    for read_only in loaded_addresses.readonly.iter() {
        account_keys_map.insert(index, read_only.parse().unwrap());
        index += 1;
    }

    account_keys_map
}

fn find_inner_instruction(
    inner_instructions: &OptionSerializer<Vec<UiInnerInstructions>>,
    instruction_index: u8,
) -> Option<UiInnerInstructions> {
    let inner_instructions = inner_instructions.clone().unwrap();
    for inner_instruction in inner_instructions.iter() {
        if inner_instruction.index == instruction_index {
            return Some(inner_instruction.clone());
        }
    }
    None
}

pub fn build_token_account_map(
    message_instructions: &[CompiledInstruction],
    meta: &UiTransactionStatusMeta,
    account_keys: &HashMap<u8, Pubkey>,
) -> HashMap<Pubkey, Pubkey> {
    // Token Account : User Account
    let mut token_account_map: HashMap<Pubkey, Pubkey> = HashMap::new();
    let inner_instructions = &meta.inner_instructions;

    for (instruction_index, instruction) in message_instructions.iter().enumerate() {
        let user_token_tuple = find_user_account(
            instruction.program_id_index,
            &instruction.accounts,
            &instruction.data,
            &account_keys,
        );
        token_account_map.insert(user_token_tuple.0, user_token_tuple.1);

        if let Some(inner_ix) = find_inner_instruction(inner_instructions, instruction_index as u8)
        {
            for inner_instruction in inner_ix.instructions.iter() {
                match inner_instruction {
                    UiInstruction::Compiled(compiled_inner_instruction) => {
                        let user_token_tuple = find_user_account(
                            compiled_inner_instruction.program_id_index,
                            &compiled_inner_instruction.accounts,
                            &compiled_inner_instruction.data,
                            &account_keys,
                        );
                        token_account_map.insert(user_token_tuple.0, user_token_tuple.1);
                    }
                    _ => {}
                }
            }
        }
    }

    let pre_token_balances = meta.pre_token_balances.clone();
    for pre in pre_token_balances.unwrap().iter() {
        let token_account = account_keys.get(&pre.account_index).unwrap();
        let user_account = pre.owner.clone().unwrap();
        token_account_map.insert(
            *token_account,
            Pubkey::from_str(user_account.as_str()).unwrap(),
        );
    }

    token_account_map
}

fn find_account_index(
    account_keys: &HashMap<u8, Pubkey>,
    account: &Pubkey,
) -> Option<u8> {
    for (index, key) in account_keys.iter() {
        if key == account {
            return Some(*index);
        }
    }
    None
}


fn find_user_account<T: AsRef<[u8]>>(
    program_id: u8,
    instruction_accounts: &Vec<u8>,
    instruction_data: T,
    account_keys: &HashMap<u8, Pubkey>,
) -> (Pubkey, Pubkey) {
    // Token Account : User Account

    let program = account_keys
        .get(&program_id)
        .unwrap_or_else(|| panic!("Program not found"));

    let instruction_bytes = instruction_data.as_ref();

    let decoded_data = match bs58::decode(instruction_bytes).into_vec() {
        Ok(data) => data,
        Err(_) => instruction_bytes.to_vec(),
    };

    if program == &Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap() {
        match TokenInstruction::unpack(&decoded_data) {
            Ok(token_instruction) => match token_instruction {
                TokenInstruction::InitializeAccount {} => {
                    return (
                        *account_keys.get(&instruction_accounts[0]).unwrap(),
                        *account_keys.get(&instruction_accounts[2]).unwrap(),
                    )
                }
                TokenInstruction::InitializeAccount2 { owner } => {
                    return (*account_keys.get(&instruction_accounts[0]).unwrap(), owner)
                }
                TokenInstruction::InitializeAccount3 { owner } => {
                    return (*account_keys.get(&instruction_accounts[0]).unwrap(), owner)
                }
                TokenInstruction::CloseAccount {} => {
                    return (
                        *account_keys.get(&instruction_accounts[0]).unwrap(),
                        *account_keys.get(&instruction_accounts[1]).unwrap(),
                    )
                }
                _ => {}
            },
            Err(e) => println!("Error unpacking token instruction: {:?}", e),
        }
    }

    if program == &Pubkey::from_str("11111111111111111111111111111111").unwrap() {
        match bincode::deserialize::<SystemInstruction>(&decoded_data) {
            Ok(system_instruction) => match system_instruction {
                SystemInstruction::CreateAccount { owner, .. } => {
                    return (*account_keys.get(&instruction_accounts[1]).unwrap(), owner)
                }
                _ => {}
            },
            Err(e) => println!("Error unpacking system instruction: {:?}", e),
        }
    }

    (Pubkey::default(), Pubkey::default())
}

pub fn find_parent_instruction(
    _message_instruction: &CompiledInstruction,
    _inner_instruction: Option<&UiInnerInstructions>,
    _inner_instruction_index: Option<u8>,
    account_keys: &HashMap<u8, Pubkey>,
) -> (Pubkey,Vec<u8>) {
    if _inner_instruction.is_some() {
        for (inner_instruction_index,inner_instruction) in _inner_instruction.unwrap().instructions.iter().rev().enumerate() {
            if inner_instruction_index as u8 <= _inner_instruction_index.unwrap() {
                match inner_instruction {
                    UiInstruction::Compiled(compiled_inner_instruction) => {
                        let program_id = account_keys.get(&compiled_inner_instruction.program_id_index).unwrap();
                        let program_accounts = compiled_inner_instruction.accounts.clone();

                        if *program_id == Pubkey::from_str(RAYDIUM_V4).unwrap() {
                            return (*program_id, program_accounts);
                        }

                    }
                    _ => {}
                }
            }
        }
    }
    let program_id = account_keys.get(&_message_instruction.program_id_index).unwrap();
    if *program_id == Pubkey::from_str(RAYDIUM_V4).unwrap() {
        return (*program_id, _message_instruction.accounts.clone());
    }

    (Pubkey::default(),Vec::new())



}
pub fn parse_instructions(
    message_instructions: &[CompiledInstruction],
    inner_instructions: &OptionSerializer<Vec<UiInnerInstructions>>,
    balance_map: &HashMap<u8, TokenBalanceDiff>,
    native_balance_map: &HashMap<u8, f64>,
    account_keys: &HashMap<u8, Pubkey>,
    token_account_map: &HashMap<Pubkey, Pubkey>,
) -> Vec<Transfer> {
    let mut transfers: Vec<Transfer> = vec![];
    for (instruction_index, instruction) in message_instructions.iter().enumerate() {
        if let Some(mut transfer) = parse_instruction(
            instruction.program_id_index,
            &instruction.accounts,
            &instruction.data,
            balance_map,
            native_balance_map,
            account_keys,
            &token_account_map,
        ) {
            let parent_instruction = find_parent_instruction(
                instruction,
                None,
                None,
                account_keys,
            );
            transfer.instruction_program_id = Some(parent_instruction.0);
            transfer.instruction_input_accounts= Some(parent_instruction.1);
            transfers.push(transfer);
        }

        if let Some(inner_ix) = find_inner_instruction(inner_instructions, instruction_index as u8)
        {
            for (inner_instruction_index, inner_instruction) in
                inner_ix.instructions.iter().enumerate()
            {
                match inner_instruction {
                    UiInstruction::Compiled(compiled_inner_instruction) => {
                        if let Some(mut transfer) = parse_instruction(
                            compiled_inner_instruction.program_id_index,
                            &compiled_inner_instruction.accounts,
                            &compiled_inner_instruction.data,
                            balance_map,
                            native_balance_map,
                            account_keys,
                            &token_account_map,
                        ) {
                            let parent_instruction =find_parent_instruction(
                                instruction,
                                Some(&inner_ix),
                                Some(inner_instruction_index as u8),
                                account_keys,
                            );
                            transfer.instruction_program_id = Some(parent_instruction.0);
                            transfer.instruction_input_accounts= Some(parent_instruction.1);
                            transfers.push(transfer);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    transfers
}

fn parse_instruction<T: AsRef<[u8]>>(
    program_id: u8,
    instruction_accounts: &Vec<u8>,
    instruction_data: T,
    balance_map: &HashMap<u8, TokenBalanceDiff>,
    native_balance_map: &HashMap<u8, f64>,
    account_keys: &HashMap<u8, Pubkey>,
    token_account_map: &HashMap<Pubkey, Pubkey>,
) -> Option<Transfer> {
    let program = account_keys
        .get(&program_id)
        .unwrap_or_else(|| panic!("Program not found"));

    if program == &Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap() {
        let instruction_bytes = instruction_data.as_ref();

        let decoded_data = match bs58::decode(instruction_bytes).into_vec() {
            Ok(data) => data,
            Err(_) => instruction_bytes.to_vec(),
        };
        match TokenInstruction::unpack(&decoded_data) {
            Ok(token_instruction) => {
                match token_instruction {
                    TokenInstruction::Transfer { amount } => {
                        let source = account_keys
                            .get(&instruction_accounts[0])
                            .cloned()
                            .unwrap_or_default();
                        let source_user =
                            token_account_map.get(&source).cloned().unwrap_or_default();
                        let destination = account_keys
                            .get(&instruction_accounts[1])
                            .cloned()
                            .unwrap_or_default();
                        let destination_user = token_account_map
                            .get(&destination)
                            .cloned()
                            .unwrap_or_default();
                        let destination_user_index = find_account_index(account_keys, &destination_user).unwrap_or_else(|| panic!("Destination user not found"));


                        let token_balance_diff = balance_map
                            .get(&instruction_accounts[1])
                            .cloned()
                            .or_else(|| {
                                native_balance_map
                                    .get(&destination_user_index)
                                    .map(|&amount| TokenBalanceDiff {
                                        token_amount: amount as f64,
                                        mint: "SOL".to_string(),
                                        decimals: 0,
                                    })
                            })
                            .unwrap_or(TokenBalanceDiff {
                                token_amount: 0.0,
                                mint: "".to_string(),
                                decimals: 0,
                            });

                        let transfer = Transfer {
                            instruction_program_id: None,
                            instruction_input_accounts: None,
                            token_balance_diff,
                            to_user_account: destination_user,
                            to_token_account: destination,
                            from_user_account: source_user,
                            from_token_account: source,
                        };
                        return Some(transfer);
                    }
                    _ => {}
                }
            }
            Err(e) => println!("Error unpacking token instruction: {:?}", e),
        }
    }

    None
}
