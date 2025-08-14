# Jito Constants for SUBLY Protocol Initialization

Your SUBLY protocol now uses configurable Jito integration! The Jito stake pool, JitoSOL mint, and SPL stake pool program addresses are set during initialization and stored in GlobalState.

## Usage

When calling the `initialize` instruction, pass these parameters:

### For Mainnet:

```typescript
const jitoStakePool = new PublicKey(
  "Jito4APyf642JPZPx3hGc6WWJ8zPKtRbRs4P815Awbb"
);
const jitoSolMint = new PublicKey(
  "J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn"
);
const splStakePoolProgram = new PublicKey(
  "SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy"
);

await program.methods
  .initialize(jitoStakePool, jitoSolMint, splStakePoolProgram)
  .accounts({
    authority: authority.publicKey,
    globalState,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

### For Devnet:

```typescript
const jitoStakePool = new PublicKey(
  "CtMyWsrUtAwXWiGr9WjHT5fC3p3fgV8cyGpLTo2LJzG1"
);
const jitoSolMint = new PublicKey(
  "Jito1StakepoLz2G3HEgoXPxntSZVyuMLgaKNJre5hPW3"
);
const splStakePoolProgram = new PublicKey(
  "SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy"
);

await program.methods
  .initialize(jitoStakePool, jitoSolMint, splStakePoolProgram)
  .accounts({
    authority: authority.publicKey,
    globalState,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

## Benefits

✅ **Network Flexibility**: Switch between mainnet/devnet without code changes
✅ **Upgradeability**: Can update Jito addresses if needed through governance
✅ **Real Integration**: Uses actual Jito SPL Stake Pool contracts
✅ **Production Ready**: Proper account validation and error handling

## How It Works

1. **Initialize**: Set Jito configuration during protocol initialization
2. **Stake**: `stake_sol` reads configuration from GlobalState and uses real Jito CPI calls
3. **Unstake**: `unstake_sol` reads configuration from GlobalState and uses real Jito CPI calls
4. **Logging**: All operations log which Jito pool is being used

The hardcoded constants have been completely removed in favor of configurable state!
