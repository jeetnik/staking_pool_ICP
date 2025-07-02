
use candid::{candid_method, Principal, CandidType, Deserialize};
use ic_cdk::api::time;
use ic_cdk_macros::{init, post_upgrade, pre_upgrade};
use ic_ledger_types::{
    AccountIdentifier, Subaccount, Tokens, DEFAULT_FEE, DEFAULT_SUBACCOUNT,
    MAINNET_LEDGER_CANISTER_ID, TransferArgs, AccountBalanceArgs,
};
use std::cell::RefCell;
use std::collections::HashMap;

mod types;
use types::*;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    users: HashMap<Principal, UserDeposits>,
    total_staked: u64,
    next_subaccount_id: u64,
    pending_deposits: HashMap<Subaccount, PendingDeposit>, 
    reward_subaccount: Option<Subaccount>, 
}

#[derive(Clone, Debug, CandidType, Deserialize)]
struct PendingDeposit {
    user: Principal,
    expected_amount: u64,
    lock_period: u64,
    created_time: u64,
}

impl State {
    fn get_user_deposits(&self, user: &Principal) -> Option<&UserDeposits> {
        self.users.get(user)
    }

    fn get_user_deposits_mut(&mut self, user: &Principal) -> &mut UserDeposits {
        self.users.entry(*user).or_insert(UserDeposits {
            deposits: Vec::new(),
        })
    }

    fn generate_subaccount(&mut self) -> Subaccount {
        let mut subaccount = [0u8; 32];
        let id_bytes = self.next_subaccount_id.to_be_bytes();
        subaccount[24..32].copy_from_slice(&id_bytes);
        self.next_subaccount_id += 1;
        Subaccount(subaccount)
    }

    fn get_reward_subaccount(&mut self) -> Subaccount {
        if let Some(subaccount) = self.reward_subaccount {
            subaccount
        } else {
            let subaccount = self.generate_subaccount();
            self.reward_subaccount = Some(subaccount);
            subaccount
        }
    }
}

#[init]
fn init() {
    ic_cdk::println!("Staking pool canister initialized");
}

//  Create deposit intention and return subaccount for user to send ICP to
#[ic_cdk::update]
#[candid_method(update)]
async fn create_deposit_intention(args: DepositArgs) -> StakingResult<DepositIntention> {
    let caller = ic_cdk::caller();
    
    if args.amount == 0 {
        return Err(StakingError::InvalidAmount);
    }

    // Generate unique subaccount for this deposit
    let subaccount = STATE.with(|s| s.borrow_mut().generate_subaccount());
    
    // Create pending deposit record
    let pending_deposit = PendingDeposit {
        user: caller,
        expected_amount: args.amount,
        lock_period: args.lock_period.to_seconds(),
        created_time: time(),
    };

    STATE.with(|s| {
        s.borrow_mut().pending_deposits.insert(subaccount, pending_deposit);
    });

    let canister_id = ic_cdk::id();
    let deposit_address = AccountIdentifier::new(&canister_id, &subaccount);

    Ok(DepositIntention {
        subaccount,
        deposit_address: deposit_address.to_string(),
        expected_amount: args.amount,
        expires_at: time() + (15 * 60 * 1_000_000_000), // 15 minutes in nanoseconds
    })
}

// Confirm deposit after user has sent ICP to the subaccount
#[ic_cdk::update]
#[candid_method(update)]
async fn confirm_deposit(subaccount: Subaccount) -> StakingResult<()> {
    let caller = ic_cdk::caller();
    
    // Get pending deposit info
    let pending_deposit = STATE.with(|s| {
        s.borrow().pending_deposits.get(&subaccount).cloned()
    }).ok_or(StakingError::DepositNotFound)?;

    // Verify caller is the one who created the deposit intention
    if pending_deposit.user != caller {
        return Err(StakingError::Unauthorized);
    }

    // Check if deposit intention has expired (15 minutes)
    let current_time = time();
    if current_time > pending_deposit.created_time + (15 * 60 * 1_000_000_000) {
        // Clean up expired deposit
        STATE.with(|s| {
            s.borrow_mut().pending_deposits.remove(&subaccount);
        });
        return Err(StakingError::DepositExpired);
    }

    // Check actual balance in the subaccount
    let canister_id = ic_cdk::id();
    let account = AccountIdentifier::new(&canister_id, &subaccount);
    
    let balance_args = AccountBalanceArgs { account };
    let balance = match ic_ledger_types::account_balance(MAINNET_LEDGER_CANISTER_ID, balance_args).await {
        Ok(balance) => balance.e8s(),
        Err(_) => return Err(StakingError::TransferFailed("Failed to check balance".to_string())),
    };

    // Verify sufficient balance (accounting for fees)
    if balance < pending_deposit.expected_amount {
        return Err(StakingError::InsufficientFunds);
    }

    // Create confirmed deposit record
    let deposit = Deposit {
        amount: balance, // Use actual balance received
        deposit_time: current_time,
        lock_period: pending_deposit.lock_period,
        subaccount,
    };

    // Store deposit and update state
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let user_deposits = state.get_user_deposits_mut(&caller);
        user_deposits.deposits.push(deposit);
        state.total_staked += balance;
        state.pending_deposits.remove(&subaccount); // Clean up pending deposit
    });

    Ok(())
}

#[ic_cdk::update]
#[candid_method(update)]
async fn withdraw(args: WithdrawArgs) -> StakingResult<u64> {
    let caller = ic_cdk::caller();
    let current_time = time();
    
    let (amount, subaccount, can_withdraw) = STATE.with(|s| {
        let state = s.borrow();
        match state.get_user_deposits(&caller) {
            Some(user_deposits) => {
                if args.deposit_index >= user_deposits.deposits.len() {
                    return (0, Subaccount([0u8; 32]), Err(StakingError::DepositNotFound));
                }
                
                let deposit = &user_deposits.deposits[args.deposit_index];
                let unlock_time = deposit.deposit_time + deposit.lock_period;
                
                if current_time < unlock_time {
                    (0, Subaccount([0u8; 32]), Err(StakingError::LockPeriodNotExpired))
                } else {
                    (deposit.amount, deposit.subaccount, Ok(()))
                }
            }
            None => (0, Subaccount([0u8; 32]), Err(StakingError::DepositNotFound)),
        }
    });

    can_withdraw?;

    // Transfer funds from deposit subaccount back to user
    let user_account = AccountIdentifier::new(&caller, &DEFAULT_SUBACCOUNT);
    let transfer_args = TransferArgs {
        memo: ic_ledger_types::Memo(0),
        amount: Tokens::from_e8s(amount.saturating_sub(DEFAULT_FEE.e8s())),
        fee: DEFAULT_FEE,
        from_subaccount: Some(subaccount),
        to: user_account,
        created_at_time: None,
    };

    match ic_ledger_types::transfer(MAINNET_LEDGER_CANISTER_ID, transfer_args).await {
        Ok(Ok(_block_height)) => {
            // Remove deposit after successful transfer
            STATE.with(|s| {
                let mut state = s.borrow_mut();
                if let Some(user_deposits) = state.users.get_mut(&caller) {
                    user_deposits.deposits.remove(args.deposit_index);
                    state.total_staked = state.total_staked.saturating_sub(amount);
                }
            });
            Ok(amount.saturating_sub(DEFAULT_FEE.e8s()))
        }
        Ok(Err(transfer_error)) => Err(StakingError::TransferFailed(format!("{:?}", transfer_error))),
        Err((code, msg)) => Err(StakingError::TransferFailed(format!("Call failed: {} - {}", code as u8, msg))),
    }
}

//  Now properly transfers ICP from reward subaccount to distribute rewards
#[ic_cdk::update]
#[candid_method(update)]
async fn reward_pool() -> StakingResult<u64> {
    let (reward_subaccount, total_staked) = STATE.with(|s| {
        let mut state = s.borrow_mut();
        (state.get_reward_subaccount(), state.total_staked)
    });

    if total_staked == 0 {
        return Ok(0);
    }

    // Check balance in reward subaccount
    let canister_id = ic_cdk::id();
    let reward_account = AccountIdentifier::new(&canister_id, &reward_subaccount);
    
    let balance_args = AccountBalanceArgs { account: reward_account };
    let reward_balance = match ic_ledger_types::account_balance(MAINNET_LEDGER_CANISTER_ID, balance_args).await {
        Ok(balance) => balance.e8s(),
        Err(_) => return Err(StakingError::TransferFailed("Failed to check reward balance".to_string())),
    };

    if reward_balance <= DEFAULT_FEE.e8s() {
        return Err(StakingError::InsufficientFunds);
    }

    let reward_amount = reward_balance.saturating_sub(DEFAULT_FEE.e8s());

    // Distribute rewards proportionally to each deposit subaccount
    let mut total_distributed = 0u64;
    
    let user_deposits_clone = STATE.with(|s| {
        s.borrow().users.clone()
    });

    for (user, user_deposits) in user_deposits_clone.iter() {
        for deposit in &user_deposits.deposits {
            let user_reward = (deposit.amount as u128 * reward_amount as u128 / total_staked as u128) as u64;
            
            if user_reward > 0 {
                // Transfer reward to user's deposit subaccount
                let deposit_account = AccountIdentifier::new(&canister_id, &deposit.subaccount);
                let transfer_args = TransferArgs {
                    memo: ic_ledger_types::Memo(1), // Reward memo
                    amount: Tokens::from_e8s(user_reward),
                    fee: DEFAULT_FEE,
                    from_subaccount: Some(reward_subaccount),
                    to: deposit_account,
                    created_at_time: None,
                };

                match ic_ledger_types::transfer(MAINNET_LEDGER_CANISTER_ID, transfer_args).await {
                    Ok(Ok(_)) => {
                        total_distributed += user_reward;
                        // Update deposit amount in state
                        STATE.with(|s| {
                            let mut state = s.borrow_mut();
                            if let Some(user_deposits_mut) = state.users.get_mut(user) {
                                for deposit_mut in &mut user_deposits_mut.deposits {
                                    if deposit_mut.subaccount == deposit.subaccount {
                                        deposit_mut.amount = deposit_mut.amount.saturating_add(user_reward);
                                        break;
                                    }
                                }
                            }
                        });
                    }
                    Ok(Err(_)) | Err(_) => {
                        // Continue with other users if one transfer fails
                        continue;
                    }
                }
            }
        }
    }

    // Update total staked
    STATE.with(|s| {
        s.borrow_mut().total_staked = s.borrow().total_staked.saturating_add(total_distributed);
    });

    Ok(total_distributed)
}

#[ic_cdk::update]
#[candid_method(update)]
async fn slash_pool(amount: u64, receiver: Principal) -> StakingResult<u64> {
    if amount == 0 {
        return Err(StakingError::InvalidAmount);
    }

    let total_staked = STATE.with(|s| s.borrow().total_staked);
    if total_staked == 0 || amount > total_staked {
        return Err(StakingError::InsufficientFunds);
    }

    let mut total_slashed = 0u64;
 

    // Collect all deposits to slash
    let user_deposits_clone = STATE.with(|s| s.borrow().users.clone());
    
    // Slash deposits proportionally by transferring from each deposit subaccount
    for (user, user_deposits) in user_deposits_clone.iter() {
        for deposit in &user_deposits.deposits {
            let slash_amount = (deposit.amount as u128 * amount as u128 / total_staked as u128) as u64;
            
            if slash_amount > DEFAULT_FEE.e8s() {
                let transfer_amount = slash_amount.saturating_sub(DEFAULT_FEE.e8s());
                let receiver_account = AccountIdentifier::new(&receiver, &DEFAULT_SUBACCOUNT);
                
                let transfer_args = TransferArgs {
                    memo: ic_ledger_types::Memo(2), // Slash memo
                    amount: Tokens::from_e8s(transfer_amount),
                    fee: DEFAULT_FEE,
                    from_subaccount: Some(deposit.subaccount),
                    to: receiver_account,
                    created_at_time: None,
                };

                match ic_ledger_types::transfer(MAINNET_LEDGER_CANISTER_ID, transfer_args).await {
                    Ok(Ok(_)) => {
                        total_slashed += slash_amount;
                        // Update deposit amount in state
                        STATE.with(|s| {
                            let mut state = s.borrow_mut();
                            if let Some(user_deposits_mut) = state.users.get_mut(user) {
                                for deposit_mut in &mut user_deposits_mut.deposits {
                                    if deposit_mut.subaccount == deposit.subaccount {
                                        deposit_mut.amount = deposit_mut.amount.saturating_sub(slash_amount);
                                        break;
                                    }
                                }
                            }
                        });
                    }
                    Ok(Err(_)) | Err(_) => {
                        // Continue with other deposits if one transfer fails
                        continue;
                    }
                }
            }
        }
    }

    // Update total staked
    STATE.with(|s| {
        s.borrow_mut().total_staked = s.borrow().total_staked.saturating_sub(total_slashed);
    });

    Ok(total_slashed)
}

// Get reward subaccount address for funding
#[ic_cdk::query]
#[candid_method(query)]
fn get_reward_address() -> String {
    let reward_subaccount = STATE.with(|s| {
        s.borrow_mut().get_reward_subaccount()
    });
    
    let canister_id = ic_cdk::id();
    let account = AccountIdentifier::new(&canister_id, &reward_subaccount);
    account.to_string()
}

// Clean up expired deposit intentions
#[ic_cdk::update]
#[candid_method(update)]
fn cleanup_expired_deposits() -> u64 {
    let current_time = time();
    let expiry_time = 15 * 60 * 1_000_000_000; // 15 minutes
    
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let initial_count = state.pending_deposits.len();
        
        state.pending_deposits.retain(|_, deposit| {
            current_time <= deposit.created_time + expiry_time
        });
        
        (initial_count - state.pending_deposits.len()) as u64
    })
}

#[ic_cdk::query]
#[candid_method(query)]
fn get_deposits(user: Principal) -> Vec<Deposit> {
    STATE.with(|s| {
        s.borrow()
            .get_user_deposits(&user)
            .map(|ud| ud.deposits.clone())
            .unwrap_or_default()
    })
}

#[ic_cdk::query]
#[candid_method(query)]
fn get_total_staked() -> u64 {
    STATE.with(|s| s.borrow().total_staked)
}

#[ic_cdk::query]
#[candid_method(query)]
fn get_deposit_address(subaccount: Subaccount) -> String {
    let canister_id = ic_cdk::id();
    let account = AccountIdentifier::new(&canister_id, &subaccount);
    account.to_string()
}

#[ic_cdk::query]
#[candid_method(query)]
fn get_pending_deposits() -> Vec<(Subaccount, PendingDeposit)> {
    STATE.with(|s| {
        s.borrow().pending_deposits.iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    })
}

// Upgrade hooks
#[pre_upgrade]
fn pre_upgrade() {
    // Serialize state for upgrade
}

#[post_upgrade]
fn post_upgrade() {
    // Deserialize state after upgrade
}

// Generate candid interface
ic_cdk::export_candid!();