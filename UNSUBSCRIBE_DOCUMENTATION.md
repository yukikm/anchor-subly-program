# User Unsubscribe Functionality

## Overview

The `unsubscribe_from_service` instruction allows users to cancel their active subscriptions. This implementation follows the flow diagram provided, ensuring proper handling of locked SOL funds and subscription certificate NFTs.

## Key Features

### 1. **Comprehensive Validation**

- Verifies the subscription is active
- Ensures the user owns the subscription
- Validates provider and service ID match
- Checks protocol is not paused

### 2. **SOL Unlocking Logic**

- Unlocks all remaining SOL that was locked for the subscription (12 months worth)
- Properly handles the case where user cancels mid-billing cycle
- Users retain access until the end of their current billing period

### 3. **NFT Certificate Management**

- Burns the subscription certificate NFT as proof of cancellation
- Validates user owns the certificate before burning
- Provides clear error if no certificate exists

### 4. **Prorated Access**

- Users keep access until their next billing cycle
- Clear messaging about remaining access time
- Handles both first-time subscriptions and recurring ones

## Function Signature

```rust
pub fn unsubscribe_from_service(
    ctx: Context<UnsubscribeFromService>,
    provider: Pubkey,
    service_id: u64,
) -> Result<()>
```

## Account Structure

```rust
pub struct UnsubscribeFromService<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut, /* user account validation */)]
    pub user_account: Account<'info, User>,

    #[account(mut, /* subscription validation */)]
    pub user_subscription: Account<'info, UserSubscription>,

    #[account(mut, /* service validation */)]
    pub subscription_service: Account<'info, SubscriptionService>,

    #[account(mut, /* provider validation */)]
    pub provider_account: Account<'info, Provider>,

    #[account(mut, /* global state */)]
    pub global_state: Account<'info, GlobalState>,

    // NFT certificate accounts
    #[account(mut)]
    pub certificate_nft_mint: Account<'info, Mint>,

    #[account(mut, /* certificate token account */)]
    pub certificate_nft_token_account: Account<'info, TokenAccount>,

    // Required programs
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
```

## Implementation Logic

### 1. **Pre-execution Validation**

```rust
// Check protocol is not paused
require!(!ctx.accounts.global_state.is_paused, ErrorCode::ProtocolPaused);

// Verify subscription is active
require!(user_subscription.is_active, ErrorCode::SubscriptionNotActive);
```

### 2. **SOL Unlocking Calculation**

```rust
// Calculate locked amount for this subscription (12 months initially locked)
let monthly_fee_lamports = subscription_service.fee_usd * 1_000_000; // Mock conversion
let locked_amount_for_subscription = monthly_fee_lamports * 12;

// Unlock all remaining SOL for this subscription
user_account.locked_sol = user_account
    .locked_sol
    .checked_sub(locked_amount_for_subscription)
    .unwrap_or(0);
```

### 3. **NFT Certificate Burning**

```rust
// Burn the subscription certificate NFT
let cpi_accounts = Burn {
    mint: ctx.accounts.certificate_nft_mint.to_account_info(),
    from: ctx.accounts.certificate_nft_token_account.to_account_info(),
    authority: ctx.accounts.user.to_account_info(),
};
let cpi_program = ctx.accounts.token_program.to_account_info();
let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
burn(cpi_ctx, 1)?;
```

### 4. **State Updates**

```rust
// Deactivate subscription
user_subscription.is_active = false;
user_subscription.unsubscribed_at = Some(current_time);

// Update counters
subscription_service.current_subscribers = subscription_service.current_subscribers.saturating_sub(1);
provider_account.total_subscribers = provider_account.total_subscribers.saturating_sub(1);
```

### 5. **Prorated Access Logic**

```rust
// Check remaining time in billing cycle
if let Some(last_payment) = user_subscription.last_payment_at {
    let time_since_payment = current_time - last_payment;
    if time_since_payment < billing_period_seconds {
        // User retains access until next billing cycle
    }
} else {
    // First billing period - access until next payment due
    let time_until_next_payment = user_subscription.next_payment_due - current_time;
    if time_until_next_payment > 0 {
        // User retains access
    }
}
```

## Error Handling

The function includes comprehensive error handling for:

- **ProtocolPaused**: When the protocol is temporarily disabled
- **SubscriptionNotActive**: When trying to unsubscribe from inactive subscription
- **UnauthorizedUser**: When user doesn't own the subscription
- **InvalidProvider/InvalidServiceId**: When provider or service doesn't match
- **NoCertificateToDestroy**: When user doesn't have the required NFT certificate

## Integration with Frontend

### Client-side Call Example

```typescript
// Unsubscribe from a service
await program.methods
  .unsubscribeFromService(providerPublicKey, serviceId)
  .accounts({
    user: userPublicKey,
    userAccount: userAccountPDA,
    userSubscription: userSubscriptionPDA,
    subscriptionService: subscriptionServicePDA,
    providerAccount: providerAccountPDA,
    globalState: globalStatePDA,
    certificateNftMint: certificateNftMint,
    certificateNftTokenAccount: certificateTokenAccount,
    tokenProgram: TOKEN_PROGRAM_ID,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

### Required Accounts Derivation

```typescript
// User subscription PDA
const [userSubscriptionPDA] = PublicKey.findProgramAddressSync(
  [
    Buffer.from("user_subscription"),
    userPublicKey.toBuffer(),
    providerPublicKey.toBuffer(),
    new BN(serviceId).toArrayLike(Buffer, "le", 8),
  ],
  programId
);

// Subscription service PDA
const [subscriptionServicePDA] = PublicKey.findProgramAddressSync(
  [
    Buffer.from("subscription_service"),
    providerPublicKey.toBuffer(),
    new BN(serviceId).toArrayLike(Buffer, "le", 8),
  ],
  programId
);
```

## Flow Diagram Implementation

This implementation follows the provided flow diagram:

1. **User Selection**: User selects unsubscribe function
2. **Get Active Subscription**: Retrieves and validates user's active subscription
3. **User Validation**: Confirms user owns the subscription (Yes path)
4. **Update Active Status**: Sets subscription `is_active` to `false`
5. **SOL Unlocking**: Frees up locked SOL for immediate availability
6. **NFT Burning**: Destroys subscription certificate as proof of cancellation
7. **Access Retention**: User keeps access until end of billing cycle

## Benefits

- **Immediate SOL Unlocking**: Users get their locked funds back immediately
- **Prorated Access**: Users don't lose access they've already paid for
- **NFT Certificate Management**: Clear proof of subscription status through NFT
- **Comprehensive Validation**: Multiple layers of security and validation
- **Counter Updates**: Maintains accurate statistics for providers and services
- **Error Handling**: Comprehensive error messages for debugging and user feedback

## Testing Considerations

When testing this functionality, ensure:

1. User has an active subscription before unsubscribing
2. User owns the subscription certificate NFT
3. Locked SOL is properly unlocked after unsubscription
4. NFT certificate is successfully burned
5. Subscription state is properly updated
6. Provider and service counters are decremented
7. User retains access for appropriate time period
8. Error cases are properly handled

This implementation provides a complete, secure, and user-friendly unsubscription system that follows best practices for Solana program development.
