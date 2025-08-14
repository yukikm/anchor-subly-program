use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

// Global seeds
pub const GLOBAL_STATE_SEED: &str = "global_state";
pub const TREASURY_SEED: &str = "treasury";

// Provider related seeds
pub const PROVIDER_SEED: &str = "provider";
pub const SUBSCRIPTION_SERVICE_SEED: &str = "subscription_service";

// User related seeds
pub const USER_SEED: &str = "user";
pub const USER_SUBSCRIPTION_SEED: &str = "user_subscription";
pub const PAYMENT_RECORD_SEED: &str = "payment_record";
pub const STAKE_ACCOUNT_SEED: &str = "stake_account";

// Vault seeds
pub const SOL_VAULT_SEED: &str = "vault";
pub const JITO_VAULT_SEED: &str = "jito_vault";

// Maximum string lengths
pub const MAX_NAME_LENGTH: usize = 64;
pub const MAX_DESCRIPTION_LENGTH: usize = 200;
pub const MAX_URL_LENGTH: usize = 200;

// Protocol configuration
pub const DEFAULT_PROTOCOL_FEE_BPS: u16 = 100; // 1%
pub const MAX_PROTOCOL_FEE_BPS: u16 = 1000; // 10%
pub const MIN_SUBSCRIPTION_PERIOD_DAYS: u64 = 7;
pub const MAX_SUBSCRIPTION_PERIOD_DAYS: u64 = 365;

// Staking configuration
pub const MIN_STAKE_AMOUNT: u64 = 1_000_000_000; // 1 SOL in lamports
pub const YIELD_CALCULATION_PERIOD: i64 = 86400; // 24 hours in seconds
