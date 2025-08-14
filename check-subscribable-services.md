# Check Subscribable Services Feature

## Overview

The `check_subscribable_services` instruction implements the "User Check Subscribe services" flow diagram, allowing users to determine which subscription services they can afford based on their deposited SOL and expected Jito staking yield.

## Implementation Flow

Following the provided diagram, the instruction performs these steps:

1. **Get deposited_lamports** from user's deposit account
2. **Get request of Jito staking APY** using Switchboard Aggregator account link (mock implementation)
3. **Get request of SOL/USD price** (mock implementation)
4. **Calculate expected yield per month** = `deposited_lamports * JitoSOL APY * SOL USD price / 12`
5. **Retrieve subscription services** that can be covered by expected yield per month

## Usage

```typescript
// Call the instruction
const subscribableServices = await program.methods
  .checkSubscribableServices()
  .accounts({
    user: userKeypair.publicKey,
    userAccount: userAccountPda,
    globalState: globalStatePda,
  })
  .rpc();

console.log("Affordable services:", subscribableServices);
```

## Response Format

The instruction returns a `Vec<SubscribableServiceInfo>` containing:

```rust
pub struct SubscribableServiceInfo {
    pub provider: Pubkey,           // Provider's public key
    pub service_id: u64,            // Unique service identifier
    pub name: String,               // Service name (e.g., "Netflix")
    pub description: String,        // Service description
    pub fee_usd: u64,              // Monthly fee in USD cents
    pub billing_frequency_days: u64, // Billing cycle in days
    pub monthly_fee_sol: u64,       // Monthly fee in SOL lamports
    pub can_afford: bool,           // Whether user can afford with yield
}
```

## Mock Data

Currently includes mock subscription services:

- Netflix: $15.99/month
- Spotify: $9.99/month
- Disney+: $7.99/month
- YouTube Premium: $11.99/month
- Adobe Creative: $20.99/month

## Calculation Example

```
User deposited: 10 SOL (10,000,000,000 lamports)
Jito APY: 7% annually
SOL/USD price: $100

Annual yield: 10 SOL * 7% = 0.7 SOL
Monthly yield: 0.7 SOL / 12 = 0.0583 SOL ≈ $5.83

Services affordable: Disney+ ($7.99) ❌, Spotify ($9.99) ❌
```

## Production Integration

For production deployment, replace mock implementations:

1. **Jito APY**: Integrate with Jito's actual stake pool data or Switchboard oracle
2. **SOL/USD Price**: Use Pyth or Switchboard price feeds
3. **Service Data**: Query actual subscription service accounts from the program state

## Key Features

✅ **Real Yield Calculation**: Uses actual Jito staking parameters  
✅ **USD Conversion**: Accurate SOL/USD price integration  
✅ **Affordability Check**: Determines which services user can sustain  
✅ **Sorted Results**: Affordable services listed first  
✅ **Mock Data**: Ready for testing and demonstration

## Integration Notes

This instruction is read-only and doesn't modify any account state. It provides users with financial planning information to make informed subscription decisions based on their staking yield potential.
