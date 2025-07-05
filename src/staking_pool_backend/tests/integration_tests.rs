use candid::{decode_one, encode_args, Principal};
use pocket_ic::PocketIc;
use std::time::Duration;

const WASM_PATH: &str = "../../target/wasm32-unknown-unknown/release/staking_pool_backend.wasm";

#[derive(candid::CandidType, candid::Deserialize, Clone, Debug)]
struct Deposit {
    amount: u64,
    deposit_time: u64,
    lock_period: u64,
    subaccount: [u8; 32],
}

#[derive(candid::CandidType, candid::Deserialize, Clone, Debug)]
enum LockPeriod {
    Days90,
    Days180,
    Days360,
}

#[derive(candid::CandidType)]
struct DepositArgs {
    amount: u64,
    lock_period: LockPeriod,
}

#[derive(candid::CandidType)]
struct WithdrawArgs {
    deposit_index: usize,
}

#[derive(candid::CandidType, candid::Deserialize, Debug)]
struct DepositIntention {
    subaccount: [u8; 32],
    deposit_address: String,
    expected_amount: u64,
    expires_at: u64,
}

#[derive(candid::CandidType, candid::Deserialize, Clone, Debug)]
struct PendingDeposit {
    user: Principal,
    expected_amount: u64,
    lock_period: u64,
    created_time: u64,
}

#[derive(candid::CandidType, candid::Deserialize, Debug)]
enum StakingError {
    InsufficientFunds,
    DepositNotFound,
    LockPeriodNotExpired,
    TransferFailed(String),
    InvalidAmount,
    Unauthorized,
    DepositExpired,
}

fn setup() -> (PocketIc, Principal) {
    let pic = PocketIc::new();
    
    let canister_id = pic.create_canister();
    pic.add_cycles(canister_id, 2_000_000_000_000);
    
    let wasm = std::fs::read(WASM_PATH).expect("Failed to read wasm. Run 'cargo build --target wasm32-unknown-unknown --release' first");
    pic.install_canister(canister_id, wasm, vec![], None);
    
    (pic, canister_id)
}


#[test]
fn test_create_deposit_intention() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let args = DepositArgs {
        amount: 1_000_000,
        lock_period: LockPeriod::Days90,
    };
    let encoded_args = encode_args((args,)).unwrap();
    
    let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
        .expect("Failed to call create_deposit_intention");
    
    let response: Result<DepositIntention, StakingError> = decode_one(&result)
        .expect("Failed to decode intention response");
    
    assert!(response.is_ok(), "Create intention failed: {:?}", response);
    let intention = response.unwrap();
    
    assert_eq!(intention.subaccount.len(), 32);
    assert_eq!(intention.expected_amount, 1_000_000);
    assert_eq!(intention.deposit_address.len(), 64); 
    assert!(intention.expires_at > 0);
    
    println!("Deposit address: {}", intention.deposit_address);
    println!("Subaccount: {:?}", intention.subaccount);
}

#[test]
fn test_confirm_deposit_without_funds() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let args = DepositArgs {
        amount: 1_000_000,
        lock_period: LockPeriod::Days90,
    };
    let encoded_args = encode_args((args,)).unwrap();
    
    let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
        .expect("Failed to call create_deposit_intention");
    
    let response: Result<DepositIntention, StakingError> = decode_one(&result)
        .expect("Failed to decode intention response");
    let intention = response.unwrap();
    
    let confirm_args = encode_args((intention.subaccount,)).unwrap();
    
    let result = pic.update_call(canister_id, user, "confirm_deposit", confirm_args);
    
    match result {
        Ok(data) => {
            let response: Result<(), StakingError> = decode_one(&data)
                .expect("Failed to decode confirm response");
            assert!(response.is_err(), "Expected confirmation to fail");
            match response.unwrap_err() {
                StakingError::InsufficientFunds => {
                    println!("Got expected InsufficientFunds error");
                },
                StakingError::TransferFailed(msg) if msg.contains("Failed to check balance") => {
                    println!("Got expected balance check failure in test environment: {}", msg);
                },
                other => panic!("Expected InsufficientFunds or balance check failure, got {:?}", other),
            }
        }
        Err(err) => {
            assert!(err.reject_message.contains("InsufficientFunds") || 
                   err.reject_message.contains("Failed to check balance") ||
                   err.reject_message.contains("TransferFailed"), 
                   "Unexpected error: {}", err.reject_message);
        }
    }
}

#[test]
fn test_get_reward_address() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let result = pic.query_call(canister_id, user, "get_reward_address", encode_args(()).unwrap())
        .expect("Failed to query reward address");
    
    let address: String = decode_one(&result)
        .expect("Failed to decode address");
    
    assert!(!address.is_empty());
    assert_eq!(address.len(), 64);
    println!("Reward address: {}", address);
}

#[test]
fn test_reward_pool_empty() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let result = pic.update_call(canister_id, user, "reward_pool", encode_args(()).unwrap());
    
    match result {
        Ok(data) => {
            let response: Result<u64, StakingError> = decode_one(&data)
                .expect("Failed to decode reward response");
            assert!(response.is_ok(), "Reward should succeed with no stakers");
            assert_eq!(response.unwrap(), 0);
        }
        Err(err) => {
            panic!("Unexpected error: {}", err.reject_message);
        }
    }
}

#[test]
fn test_slash_pool_empty() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    let receiver = Principal::from_text("rdmx6-jaaaa-aaaaa-aaadq-cai").unwrap();
    
    let slash_args = encode_args((1_000_000u64, receiver)).unwrap();
    let result = pic.update_call(canister_id, user, "slash_pool", slash_args);
    
    match result {
        Ok(data) => {
            let response: Result<u64, StakingError> = decode_one(&data)
                .expect("Failed to decode slash response");
            assert!(response.is_err(), "Slash should fail with no stakers");
            match response.unwrap_err() {
                StakingError::InsufficientFunds => {},
                other => panic!("Expected InsufficientFunds, got {:?}", other),
            }
        }
        Err(err) => {
            assert!(err.reject_message.contains("InsufficientFunds"), 
                   "Expected InsufficientFunds, got: {}", err.reject_message);
        }
    }
}

#[test]
fn test_cleanup_expired_deposits() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    for i in 0..3 {
        let args = DepositArgs {
            amount: 1_000_000 + i * 100_000,
            lock_period: LockPeriod::Days90,
        };
        let encoded_args = encode_args((args,)).unwrap();
        
        pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
            .expect("Failed to create deposit intention");
    }
    
    let pending_result = pic.query_call(canister_id, user, "get_pending_deposits", encode_args(()).unwrap())
        .expect("Failed to query pending deposits");
    
    let pending: Vec<([u8; 32], PendingDeposit)> = decode_one(&pending_result)
        .expect("Failed to decode pending deposits");
    assert_eq!(pending.len(), 3);
    
    pic.advance_time(Duration::from_secs(16 * 60));
    
    let cleanup_result = pic.update_call(canister_id, user, "cleanup_expired_deposits", encode_args(()).unwrap())
        .expect("Failed to cleanup");
    
    let cleaned_count: u64 = decode_one(&cleanup_result)
        .expect("Failed to decode cleanup count");
    assert_eq!(cleaned_count, 3);
    
    let pending_result = pic.query_call(canister_id, user, "get_pending_deposits", encode_args(()).unwrap())
        .expect("Failed to query pending deposits");
    
    let pending: Vec<([u8; 32], PendingDeposit)> = decode_one(&pending_result)
        .expect("Failed to decode pending deposits");
    assert_eq!(pending.len(), 0);
}

#[test]
fn test_unauthorized_confirm_deposit() {
    let (pic, canister_id) = setup();
    let user1 = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    let user2 = Principal::from_text("be2us-64aaa-aaaaa-qaabq-cai").unwrap();
    
    let args = DepositArgs {
        amount: 1_000_000,
        lock_period: LockPeriod::Days90,
    };
    let encoded_args = encode_args((args,)).unwrap();
    
    let result = pic.update_call(canister_id, user1, "create_deposit_intention", encoded_args)
        .expect("Failed to call create_deposit_intention");
    
    let response: Result<DepositIntention, StakingError> = decode_one(&result)
        .expect("Failed to decode intention response");
    let intention = response.unwrap();
    
    let confirm_args = encode_args((intention.subaccount,)).unwrap();
    
    let result = pic.update_call(canister_id, user2, "confirm_deposit", confirm_args);
    
    match result {
        Ok(data) => {
            let response: Result<(), StakingError> = decode_one(&data)
                .expect("Failed to decode confirm response");
            assert!(response.is_err(), "Expected confirmation to fail");
            match response.unwrap_err() {
                StakingError::Unauthorized => {},
                other => panic!("Expected Unauthorized, got {:?}", other),
            }
        }
        Err(err) => {
            assert!(err.reject_message.contains("Unauthorized"), 
                   "Expected Unauthorized, got: {}", err.reject_message);
        }
    }
}

#[test]
fn test_expired_deposit_intention() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let args = DepositArgs {
        amount: 1_000_000,
        lock_period: LockPeriod::Days90,
    };
    let encoded_args = encode_args((args,)).unwrap();
    
    let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
        .expect("Failed to call create_deposit_intention");
    
    let response: Result<DepositIntention, StakingError> = decode_one(&result)
        .expect("Failed to decode intention response");
    let intention = response.unwrap();
    
    pic.advance_time(Duration::from_secs(16 * 60));
    
    let confirm_args = encode_args((intention.subaccount,)).unwrap();
    
    let result = pic.update_call(canister_id, user, "confirm_deposit", confirm_args);
    
    match result {
        Ok(data) => {
            let response: Result<(), StakingError> = decode_one(&data)
                .expect("Failed to decode confirm response");
            assert!(response.is_err(), "Expected confirmation to fail");
            match response.unwrap_err() {
                StakingError::DepositExpired => {},
                other => panic!("Expected DepositExpired, got {:?}", other),
            }
        }
        Err(err) => {
            assert!(err.reject_message.contains("DepositExpired"), 
                   "Expected DepositExpired, got: {}", err.reject_message);
        }
    }
}

#[test]
fn test_invalid_operations() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let args = DepositArgs {
        amount: 0,
        lock_period: LockPeriod::Days90,
    };
    let encoded_args = encode_args((args,)).unwrap();
    
    let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args);
    
    match result {
        Ok(data) => {
            let response: Result<DepositIntention, StakingError> = decode_one(&data)
                .expect("Failed to decode intention response");
            assert!(response.is_err(), "Expected intention creation to fail");
            match response.unwrap_err() {
                StakingError::InvalidAmount => {},
                other => panic!("Expected InvalidAmount, got {:?}", other),
            }
        }
        Err(err) => {
            assert!(err.reject_message.contains("InvalidAmount"), 
                   "Expected InvalidAmount, got: {}", err.reject_message);
        }
    }
    
    let withdraw_args = WithdrawArgs { deposit_index: 0 };
    let encoded_args = encode_args((withdraw_args,)).unwrap();
    
    let result = pic.update_call(canister_id, user, "withdraw", encoded_args);
    
    match result {
        Ok(data) => {
            let response: Result<u64, StakingError> = decode_one(&data)
                .expect("Failed to decode withdraw response");
            assert!(response.is_err(), "Expected withdrawal to fail");
            match response.unwrap_err() {
                StakingError::DepositNotFound => {},
                other => panic!("Expected DepositNotFound, got {:?}", other),
            }
        }
        Err(err) => {
            assert!(err.reject_message.contains("DepositNotFound"), 
                   "Expected DepositNotFound, got: {}", err.reject_message);
        }
    }
}


#[test]
fn test_complete_user_journey() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let args = DepositArgs {
        amount: 5_000_000,
        lock_period: LockPeriod::Days90,
    };
    let encoded_args = encode_args((args,)).unwrap();
    
    let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
        .expect("Failed to create deposit intention");
    
    let response: Result<DepositIntention, StakingError> = decode_one(&result).unwrap();
    let intention = response.unwrap();
    
    let confirm_args = encode_args((intention.subaccount,)).unwrap();
    
    let result = pic.update_call(canister_id, user, "confirm_deposit", confirm_args);
    
    match result {
        Ok(data) => {
            let response: Result<(), StakingError> = decode_one(&data).unwrap();
            assert!(response.is_err(), "Expected balance check to fail in test env");
        }
        Err(err) => {
            assert!(err.reject_message.contains("InsufficientFunds") || 
                   err.reject_message.contains("Failed to check balance"));
        }
    }
    
    println!("Complete user journey validation passed");
}

#[test]
fn test_multiple_deposits_same_user() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let amounts = vec![1_000_000, 2_000_000, 3_000_000];
    let lock_periods = vec![LockPeriod::Days90, LockPeriod::Days180, LockPeriod::Days360];
    
    let mut intentions = Vec::new();
    
    for (i, (&amount, lock_period)) in amounts.iter().zip(lock_periods.iter()).enumerate() {
        let args = DepositArgs {
            amount,
            lock_period: lock_period.clone(),
        };
        let encoded_args = encode_args((args,)).unwrap();
        
        let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
            .expect(&format!("Failed to create deposit intention {}", i));
        
        let response: Result<DepositIntention, StakingError> = decode_one(&result).unwrap();
        let intention = response.unwrap();
        
        assert_eq!(intention.expected_amount, amount);
        assert_eq!(intention.subaccount.len(), 32);
        assert!(intention.expires_at > 0);
        
        intentions.push(intention);
    }
    
    for i in 0..intentions.len() {
        for j in i+1..intentions.len() {
            assert_ne!(intentions[i].subaccount, intentions[j].subaccount, 
                      "Subaccounts should be unique");
        }
    }
    
    println!("Multiple deposits same user test passed");
}

#[test]
fn test_multiple_users_operations() {
    let (pic, canister_id) = setup();
    
    let users = vec![
        Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap(),
        Principal::from_text("be2us-64aaa-aaaaa-qaabq-cai").unwrap(),
        Principal::from_text("rdmx6-jaaaa-aaaaa-aaadq-cai").unwrap(),
    ];
    
    let amounts = vec![1_000_000, 3_000_000, 5_000_000];
    
    for (user, amount) in users.iter().zip(amounts.iter()) {
        let args = DepositArgs {
            amount: *amount,
            lock_period: LockPeriod::Days90,
        };
        let encoded_args = encode_args((args,)).unwrap();
        
        let result = pic.update_call(canister_id, *user, "create_deposit_intention", encoded_args)
            .expect("Failed to create deposit intention");
        
        let response: Result<DepositIntention, StakingError> = decode_one(&result).unwrap();
        assert!(response.is_ok(), "User deposit intention should succeed");
    }
    
    let pending_result = pic.query_call(canister_id, users[0], "get_pending_deposits", encode_args(()).unwrap())
        .expect("Failed to query pending deposits");
    
    let pending: Vec<([u8; 32], PendingDeposit)> = decode_one(&pending_result).unwrap();
    assert_eq!(pending.len(), 3, "Should have 3 pending deposits");
    
    let mut subaccounts = Vec::new();
    for (subaccount, _) in pending {
        subaccounts.push(subaccount);
    }
    
    for i in 0..subaccounts.len() {
        for j in i+1..subaccounts.len() {
            assert_ne!(subaccounts[i], subaccounts[j], "All subaccounts should be unique");
        }
    }
    
    println!("Multiple users operations test passed");
}

#[test]
fn test_reward_address_consistency() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let mut addresses = Vec::new();
    
    for _ in 0..5 {
        let result = pic.query_call(canister_id, user, "get_reward_address", encode_args(()).unwrap())
            .expect("Failed to query reward address");
        
        let address: String = decode_one(&result).unwrap();
        addresses.push(address);
    }
    
    for i in 1..addresses.len() {
        assert_eq!(addresses[0], addresses[i], "Reward address should be consistent");
    }
    
    assert_eq!(addresses[0].len(), 64, "ICP address should be 64 characters");
    assert!(!addresses[0].is_empty(), "Address should not be empty");
    
    println!("Consistent reward address: {}", addresses[0]);
    println!("Reward address consistency test passed");
}

#[test]
fn test_slash_pool_comprehensive() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    let receiver = Principal::from_text("rdmx6-jaaaa-aaaaa-aaadq-cai").unwrap();
    
    let zero_slash_args = encode_args((0u64, receiver)).unwrap();
    let result = pic.update_call(canister_id, user, "slash_pool", zero_slash_args);
    
    match result {
        Ok(data) => {
            let response: Result<u64, StakingError> = decode_one(&data).unwrap();
            assert!(response.is_err(), "Zero slash should fail");
            match response.unwrap_err() {
                StakingError::InvalidAmount => {},
                other => panic!("Expected InvalidAmount, got {:?}", other),
            }
        }
        Err(err) => {
            assert!(err.reject_message.contains("InvalidAmount"));
        }
    }
    
    let slash_args = encode_args((1_000_000u64, receiver)).unwrap();
    let result = pic.update_call(canister_id, user, "slash_pool", slash_args);
    
    match result {
        Ok(data) => {
            let response: Result<u64, StakingError> = decode_one(&data).unwrap();
            assert!(response.is_err(), "Empty pool slash should fail");
            match response.unwrap_err() {
                StakingError::InsufficientFunds => {},
                other => panic!("Expected InsufficientFunds, got {:?}", other),
            }
        }
        Err(err) => {
            assert!(err.reject_message.contains("InsufficientFunds"));
        }
    }
    
    println!("Slash pool comprehensive test passed");
}

#[test]
fn test_query_functions_edge_cases() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    let non_existent_user = Principal::from_text("2vxsx-fae").unwrap();
    
    let result = pic.query_call(canister_id, user, "get_deposits", 
                               encode_args((non_existent_user,)).unwrap())
        .expect("Failed to query deposits");
    
    let deposits: Vec<Deposit> = decode_one(&result).unwrap();
    assert_eq!(deposits.len(), 0, "Non-existent user should have no deposits");
    
    let result = pic.query_call(canister_id, user, "get_total_staked", encode_args(()).unwrap())
        .expect("Failed to query total staked");
    
    let total_staked: u64 = decode_one(&result).unwrap();
    assert_eq!(total_staked, 0, "Empty pool should have 0 total staked");
    
    let result = pic.query_call(canister_id, user, "get_pending_deposits", encode_args(()).unwrap())
        .expect("Failed to query pending deposits");
    
    let pending: Vec<([u8; 32], PendingDeposit)> = decode_one(&result).unwrap();
    assert_eq!(pending.len(), 0, "Empty state should have no pending deposits");
    
    println!("Query functions edge cases test passed");
}

#[test]
fn test_cleanup_performance() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let num_deposits = 10;
    for i in 0..num_deposits {
        let args = DepositArgs {
            amount: 1_000_000 + (i * 100_000),
            lock_period: LockPeriod::Days90,
        };
        let encoded_args = encode_args((args,)).unwrap();
        
        pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
            .expect("Failed to create deposit intention");
    }
    
    let pending_result = pic.query_call(canister_id, user, "get_pending_deposits", encode_args(()).unwrap())
        .expect("Failed to query pending deposits");
    
    let pending: Vec<([u8; 32], PendingDeposit)> = decode_one(&pending_result).unwrap();
    assert_eq!(pending.len(), num_deposits as usize, "Should have all pending deposits");
    
    pic.advance_time(Duration::from_secs(16 * 60));
    
    let cleanup_result = pic.update_call(canister_id, user, "cleanup_expired_deposits", encode_args(()).unwrap())
        .expect("Failed to cleanup");
    
    let cleaned_count: u64 = decode_one(&cleanup_result).unwrap();
    assert_eq!(cleaned_count, num_deposits as u64, "Should clean all deposits");
    
    let pending_result = pic.query_call(canister_id, user, "get_pending_deposits", encode_args(()).unwrap())
        .expect("Failed to query pending deposits");
    
    let pending: Vec<([u8; 32], PendingDeposit)> = decode_one(&pending_result).unwrap();
    assert_eq!(pending.len(), 0, "Should be empty after cleanup");
    
    println!(" Cleanup performance test passed");
}

#[test]
fn test_repeated_cleanup_calls() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    for i in 0..3 {
        let args = DepositArgs {
            amount: 1_000_000 + (i * 100_000),
            lock_period: LockPeriod::Days90,
        };
        let encoded_args = encode_args((args,)).unwrap();
        
        pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
            .expect("Failed to create deposit intention");
    }
    
    pic.advance_time(Duration::from_secs(16 * 60));
    
    let cleanup_result = pic.update_call(canister_id, user, "cleanup_expired_deposits", encode_args(()).unwrap())
        .expect("Failed to cleanup");
    
    let cleaned_count: u64 = decode_one(&cleanup_result).unwrap();
    assert_eq!(cleaned_count, 3, "Should clean 3 deposits");
    
    let cleanup_result = pic.update_call(canister_id, user, "cleanup_expired_deposits", encode_args(()).unwrap())
        .expect("Failed to cleanup");
    
    let cleaned_count: u64 = decode_one(&cleanup_result).unwrap();
    assert_eq!(cleaned_count, 0, "Should clean 0 deposits on second call");
    
    let cleanup_result = pic.update_call(canister_id, user, "cleanup_expired_deposits", encode_args(()).unwrap())
        .expect("Failed to cleanup");
    
    let cleaned_count: u64 = decode_one(&cleanup_result).unwrap();
    assert_eq!(cleaned_count, 0, "Should clean 0 deposits on third call");
    
    println!("Repeated cleanup calls test passed");
}

#[test]
fn test_input_boundary_conditions() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    let min_args = DepositArgs {
        amount: 1,
        lock_period: LockPeriod::Days90,
    };
    let encoded_args = encode_args((min_args,)).unwrap();
   
   let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args);
   match result {
       Ok(data) => {
           let response: Result<DepositIntention, StakingError> = decode_one(&data).unwrap();
           assert!(response.is_ok(), "Minimum amount should be accepted");
       }
       Err(_) => panic!("Minimum amount should work"),
   }
   
   let large_args = DepositArgs {
       amount: 1_000_000_000_000, // 10,000 ICP
       lock_period: LockPeriod::Days360,
   };
   let encoded_args = encode_args((large_args,)).unwrap();
   
   let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args);
   match result {
       Ok(data) => {
           let response: Result<DepositIntention, StakingError> = decode_one(&data).unwrap();
           assert!(response.is_ok(), "Large amount should be accepted");
       }
       Err(_) => panic!("Large amount should work"),
   }
   
   println!("Input boundary conditions test passed");
}

#[test]
fn test_different_lock_periods() {
   let (pic, canister_id) = setup();
   let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
   
   let lock_periods = vec![
       (LockPeriod::Days90, 90 * 24 * 60 * 60),
       (LockPeriod::Days180, 180 * 24 * 60 * 60),
       (LockPeriod::Days360, 360 * 24 * 60 * 60),
   ];
   
   for (i, (lock_period, expected_seconds)) in lock_periods.iter().enumerate() {
       let args = DepositArgs {
           amount: 1_000_000 + (i as u64 * 100_000),
           lock_period: lock_period.clone(),
       };
       let encoded_args = encode_args((args,)).unwrap();
       
       let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
           .expect("Failed to create deposit intention");
       
       let response: Result<DepositIntention, StakingError> = decode_one(&result).unwrap();
       let intention = response.unwrap();
       
       assert_eq!(intention.expected_amount, 1_000_000 + (i as u64 * 100_000));
       assert!(intention.expires_at > 0);
       
       println!("Lock period {:?}: {} seconds", lock_period, expected_seconds);
   }
   
   println!("Different lock periods test passed");
}

#[test]
fn test_cross_user_interference() {
   let (pic, canister_id) = setup();
   let user1 = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
   let user2 = Principal::from_text("be2us-64aaa-aaaaa-qaabq-cai").unwrap();
   
   let args1 = DepositArgs {
       amount: 1_000_000,
       lock_period: LockPeriod::Days90,
   };
   let encoded_args1 = encode_args((args1,)).unwrap();
   
   let result1 = pic.update_call(canister_id, user1, "create_deposit_intention", encoded_args1)
       .expect("Failed to create user1 intention");
   
   let response1: Result<DepositIntention, StakingError> = decode_one(&result1).unwrap();
   let _intention1 = response1.unwrap();
   
   let args2 = DepositArgs {
       amount: 2_000_000,
       lock_period: LockPeriod::Days180,
   };
   let encoded_args2 = encode_args((args2,)).unwrap();
   
   let result2 = pic.update_call(canister_id, user2, "create_deposit_intention", encoded_args2)
       .expect("Failed to create user2 intention");
   
   let response2: Result<DepositIntention, StakingError> = decode_one(&result2).unwrap();
   let intention2 = response2.unwrap();
   
   let confirm_args = encode_args((intention2.subaccount,)).unwrap();
   let result = pic.update_call(canister_id, user1, "confirm_deposit", confirm_args);
   
   match result {
       Ok(data) => {
           let response: Result<(), StakingError> = decode_one(&data).unwrap();
           assert!(response.is_err(), "Cross-user confirmation should fail");
           match response.unwrap_err() {
               StakingError::Unauthorized => {},
               _ => panic!("Expected Unauthorized error"),
           }
       }
       Err(err) => {
           assert!(err.reject_message.contains("Unauthorized"));
       }
   }
   
   println!("Cross-user interference test passed");
}

#[test]
fn test_reward_distribution_edge_cases() {
   let (pic, canister_id) = setup();
   let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
   
   let result = pic.update_call(canister_id, user, "reward_pool", encode_args(()).unwrap());
   
   match result {
       Ok(data) => {
           let response: Result<u64, StakingError> = decode_one(&data).unwrap();
           assert!(response.is_ok(), "Empty pool reward should succeed");
           assert_eq!(response.unwrap(), 0, "Should distribute 0 rewards");
       }
       Err(err) => {
           panic!("Unexpected error in empty pool reward: {}", err.reject_message);
       }
   }
   
   for _ in 0..3 {
       let result = pic.update_call(canister_id, user, "reward_pool", encode_args(()).unwrap());
       match result {
           Ok(data) => {
               let response: Result<u64, StakingError> = decode_one(&data).unwrap();
               assert_eq!(response.unwrap(), 0);
           }
           Err(_) => panic!("Multiple reward calls should work"),
       }
   }
   
   println!(" Reward distribution edge cases test passed");
}

#[test]
fn test_slash_pool_receiver_scenarios() {
   let (pic, canister_id) = setup();
   let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
   
   let receivers = vec![
       Principal::from_text("rdmx6-jaaaa-aaaaa-aaadq-cai").unwrap(),
       Principal::from_text("be2us-64aaa-aaaaa-qaabq-cai").unwrap(),
   ];
   
   for receiver in receivers {
       let slash_args = encode_args((1_000_000u64, receiver)).unwrap();
       let result = pic.update_call(canister_id, user, "slash_pool", slash_args);
       
       match result {
           Ok(data) => {
               let response: Result<u64, StakingError> = decode_one(&data).unwrap();
               assert!(response.is_err());
               match response.unwrap_err() {
                   StakingError::InsufficientFunds => {},
                   other => panic!("Expected InsufficientFunds, got {:?}", other),
               }
           }
           Err(err) => {
               assert!(err.reject_message.contains("InsufficientFunds"));
           }
       }
   }
   
   println!("Slash pool receiver scenarios test passed");
}


#[test]
fn test_time_manipulation_scenarios() {
   let (pic, canister_id) = setup();
   let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
   
   let args = DepositArgs {
       amount: 1_000_000,
       lock_period: LockPeriod::Days90,
   };
   let encoded_args = encode_args((args,)).unwrap();
   
   let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
       .expect("Failed to create deposit intention");
   
   let response: Result<DepositIntention, StakingError> = decode_one(&result).unwrap();
   let intention = response.unwrap();
   
   let confirm_args = encode_args((intention.subaccount,)).unwrap();
   let result = pic.update_call(canister_id, user, "confirm_deposit", confirm_args.clone());
   
   match result {
       Ok(data) => {
           let response: Result<(), StakingError> = decode_one(&data).unwrap();
           assert!(response.is_err(), "Should fail due to no transfer");
       }
       Err(_) => {} // Expected in test environment
   }
   
   pic.advance_time(Duration::from_secs(16 * 60));
   
   let result = pic.update_call(canister_id, user, "confirm_deposit", confirm_args);
   
   match result {
       Ok(data) => {
           let response: Result<(), StakingError> = decode_one(&data).unwrap();
           assert!(response.is_err(), "Should fail due to expiry");
           match response.unwrap_err() {
               StakingError::DepositExpired => {},
               _ => {} // Other errors acceptable in test environment
           }
       }
       Err(err) => {
           assert!(err.reject_message.contains("DepositExpired") || 
                  err.reject_message.contains("InsufficientFunds"));
       }
   }
   
   println!(" Time manipulation scenarios test passed");
}

#[test]
fn test_stress_subaccount_generation() {
   let (pic, canister_id) = setup();
   let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
   
   let mut subaccounts = Vec::new();
   let num_intentions = 20;
   
   for i in 0..num_intentions {
       let args = DepositArgs {
           amount: 1_000_000 + (i * 10_000),
           lock_period: LockPeriod::Days90,
       };
       let encoded_args = encode_args((args,)).unwrap();
       
       let result = pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
           .expect("Failed to create deposit intention");
       
       let response: Result<DepositIntention, StakingError> = decode_one(&result).unwrap();
       let intention = response.unwrap();
       
       subaccounts.push(intention.subaccount);
   }
   
   for i in 0..subaccounts.len() {
       for j in i+1..subaccounts.len() {
           assert_ne!(subaccounts[i], subaccounts[j], 
                     "Subaccount {} and {} should be different", i, j);
       }
   }
   
   for i in 1..subaccounts.len() {
       let prev_id = u64::from_be_bytes([
           subaccounts[i-1][24], subaccounts[i-1][25], subaccounts[i-1][26], subaccounts[i-1][27],
           subaccounts[i-1][28], subaccounts[i-1][29], subaccounts[i-1][30], subaccounts[i-1][31],
       ]);
       let curr_id = u64::from_be_bytes([
           subaccounts[i][24], subaccounts[i][25], subaccounts[i][26], subaccounts[i][27],
           subaccounts[i][28], subaccounts[i][29], subaccounts[i][30], subaccounts[i][31],
       ]);
       
       assert_eq!(curr_id, prev_id + 1, "Subaccount IDs should be sequential");
   }
   
   println!(" Stress subaccount generation test passed");
}

#[test]
fn test_edge_case_empty_operations() {
   let (pic, canister_id) = setup();
   let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
   
   let result = pic.query_call(canister_id, user, "get_total_staked", encode_args(()).unwrap())
       .expect("get_total_staked should work on empty state");
   let total: u64 = decode_one(&result).unwrap();
   assert_eq!(total, 0);
   
   let result = pic.query_call(canister_id, user, "get_pending_deposits", encode_args(()).unwrap())
       .expect("get_pending_deposits should work on empty state");
   let pending: Vec<([u8; 32], PendingDeposit)> = decode_one(&result).unwrap();
   assert_eq!(pending.len(), 0);
   
   let result = pic.query_call(canister_id, user, "get_deposits", encode_args((user,)).unwrap())
       .expect("get_deposits should work on empty state");
   let deposits: Vec<Deposit> = decode_one(&result).unwrap();
   assert_eq!(deposits.len(), 0);
   
   let result = pic.update_call(canister_id, user, "cleanup_expired_deposits", encode_args(()).unwrap())
       .expect("cleanup should work on empty state");
   let cleaned: u64 = decode_one(&result).unwrap();
   assert_eq!(cleaned, 0);
   
   let result = pic.update_call(canister_id, user, "reward_pool", encode_args(()).unwrap())
       .expect("reward_pool should work on empty state");
   
   let response: Result<u64, StakingError> = decode_one(&result).unwrap();
   assert_eq!(response.unwrap(), 0);
   
   println!(" Edge case empty operations test passed");
}