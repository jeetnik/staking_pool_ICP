
use candid::{CandidType, Deserialize};
use ic_ledger_types::Subaccount;
use serde::Serialize;

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct Deposit {
    pub amount: u64,
    pub deposit_time: u64,
    pub lock_period: u64, // in seconds
    pub subaccount: Subaccount,
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct UserDeposits {
    pub deposits: Vec<Deposit>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum LockPeriod {
    Days90,
    Days180,
    Days360,
}

impl LockPeriod {
    pub fn to_seconds(&self) -> u64 {
        match self {
            LockPeriod::Days90 => 90 * 24 * 60 * 60,
            LockPeriod::Days180 => 180 * 24 * 60 * 60,
            LockPeriod::Days360 => 360 * 24 * 60 * 60,
        }
    }
}

#[derive(CandidType, Deserialize, Debug)]
pub struct DepositArgs {
    pub amount: u64,
    pub lock_period: LockPeriod,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct WithdrawArgs {
    pub deposit_index: usize,
}


#[derive(CandidType, Deserialize, Debug)]
pub struct DepositIntention {
    pub subaccount: Subaccount,
    pub deposit_address: String,
    pub expected_amount: u64,
    pub expires_at: u64, 
}

#[derive(CandidType, Deserialize, Debug)]
pub enum StakingError {
    InsufficientFunds,
    DepositNotFound,
    LockPeriodNotExpired,
    TransferFailed(String),
    InvalidAmount,
    Unauthorized,
    DepositExpired, 
}

pub type StakingResult<T> = Result<T, StakingError>;