
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

#[derive(candid::CandidType, candid::Deserialize)]
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
    
    // Create deposit intention
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
    
    // Try to confirm deposit without sending ICP (should fail)
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
    assert_eq!(address.len(), 64); // ICP address length
    println!("Reward address: {}", address);
}

#[test]
fn test_reward_pool_empty() {
    let (pic, canister_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    
    // Try to distribute rewards with no stakers
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
    
    // Try to slash with no stakers
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
    
    // Create multiple deposit intentions
    for i in 0..3 {
        let args = DepositArgs {
            amount: 1_000_000 + i * 100_000,
            lock_period: LockPeriod::Days90,
        };
        let encoded_args = encode_args((args,)).unwrap();
        
        pic.update_call(canister_id, user, "create_deposit_intention", encoded_args)
            .expect("Failed to create deposit intention");
    }
    
    // Check pending deposits
    let pending_result = pic.query_call(canister_id, user, "get_pending_deposits", encode_args(()).unwrap())
        .expect("Failed to query pending deposits");
    
    let pending: Vec<([u8; 32], PendingDeposit)> = decode_one(&pending_result)
        .expect("Failed to decode pending deposits");
    assert_eq!(pending.len(), 3);
    
    // Advance time by 16 minutes (past expiry)
    pic.advance_time(Duration::from_secs(16 * 60));
    
    // Cleanup expired deposits
    let cleanup_result = pic.update_call(canister_id, user, "cleanup_expired_deposits", encode_args(()).unwrap())
        .expect("Failed to cleanup");
    
    let cleaned_count: u64 = decode_one(&cleanup_result)
        .expect("Failed to decode cleanup count");
    assert_eq!(cleaned_count, 3);
    
    // Verify all pending deposits are gone
    let pending_result = pic.query_call(canister_id, user, "get_pending_deposits", encode_args(()).unwrap())
        .expect("Failed to query pending deposits");
    
    let pending: Vec<([u8; 32], PendingDeposit)> = decode_one(&pending_result)
        .expect("Failed to decode pending deposits");
    assert_eq!(pending.len(), 0);
}

#[derive(candid::CandidType, candid::Deserialize, Clone, Debug)]
struct PendingDeposit {
    user: Principal,
    expected_amount: u64,
    lock_period: u64,
    created_time: u64,
}

#[test]
fn test_unauthorized_confirm_deposit() {
    let (pic, canister_id) = setup();
    let user1 = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    let user2 = Principal::from_text("be2us-64aaa-aaaaa-qaabq-cai").unwrap();
    
    // User1 creates deposit intention
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
    
    // User2 tries to confirm User1's deposit (should fail)
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
    
    // Create deposit intention
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
    
    // Advance time by 16 minutes (past expiry)
    pic.advance_time(Duration::from_secs(16 * 60));
    
    // Try to confirm expired deposit
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
    
    // Test zero amount deposit intention
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
    
    // Test withdraw non-existent deposit
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


    