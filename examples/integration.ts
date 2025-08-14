import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SublyProgram } from "../target/types/subly_program";
import {
  PublicKey,
  Keypair,
  LAMPORTS_PER_SOL,
  Connection,
} from "@solana/web3.js";

/**
 * SUBLY Protocol Integration Example
 *
 * This script demonstrates how to integrate with the SUBLY protocol
 * for both providers and users.
 */

export class SublyIntegration {
  private program: Program<SublyProgram>;
  private provider: anchor.AnchorProvider;

  constructor(programId: string, rpcUrl: string, wallet: any) {
    const connection = new Connection(rpcUrl, "confirmed");
    this.provider = new anchor.AnchorProvider(connection, wallet, {
      commitment: "confirmed",
    });
    anchor.setProvider(this.provider);

    this.program = new Program(
      require("../target/idl/subly_program.json"),
      new PublicKey(programId),
      this.provider
    );
  }

  // ===== PROTOCOL MANAGEMENT =====

  /**
   * Initialize the SUBLY protocol (Authority only)
   */
  async initializeProtocol(authority: Keypair): Promise<string> {
    const [globalStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("global_state")],
      this.program.programId
    );

    const tx = await this.program.methods
      .initialize()
      .accounts({
        authority: authority.publicKey,
        globalState: globalStatePda,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    console.log("✅ Protocol initialized:", tx);
    return tx;
  }

  /**
   * Get protocol statistics
   */
  async getProtocolStats(): Promise<any> {
    const [globalStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("global_state")],
      this.program.programId
    );

    const globalState = await this.program.account.globalState.fetch(
      globalStatePda
    );
    return {
      totalProviders: globalState.totalProviders.toNumber(),
      totalUsers: globalState.totalUsers.toNumber(),
      totalServices: globalState.totalSubscriptionServices.toNumber(),
      activeSubscriptions: globalState.totalActiveSubscriptions.toNumber(),
      protocolFee: globalState.protocolFeeBps / 100,
      isPaused: globalState.isPaused,
    };
  }

  // ===== PROVIDER FUNCTIONS =====

  /**
   * Register as a service provider
   */
  async registerProvider(
    provider: Keypair,
    name: string,
    description: string,
    website: string
  ): Promise<string> {
    const [globalStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("global_state")],
      this.program.programId
    );

    const [providerPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider"), provider.publicKey.toBuffer()],
      this.program.programId
    );

    const providerNftMint = Keypair.generate();
    const [providerNftTokenAccount] = PublicKey.findProgramAddressSync(
      [
        provider.publicKey.toBuffer(),
        anchor.utils.token.TOKEN_PROGRAM_ID.toBuffer(),
        providerNftMint.publicKey.toBuffer(),
      ],
      anchor.utils.token.ASSOCIATED_PROGRAM_ID
    );

    const tx = await this.program.methods
      .registerProvider(name, description, website)
      .accounts({
        provider: provider.publicKey,
        globalState: globalStatePda,
        providerAccount: providerPda,
        providerNftMint: providerNftMint.publicKey,
        providerNftTokenAccount: providerNftTokenAccount,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([provider, providerNftMint])
      .rpc();

    console.log(`✅ Provider '${name}' registered:`, tx);
    return tx;
  }

  /**
   * Create a subscription service
   */
  async createSubscriptionService(
    provider: Keypair,
    name: string,
    description: string,
    feeUsd: number, // In cents
    billingDays: number,
    imageUrl: string,
    maxSubscribers?: number
  ): Promise<string> {
    const [globalStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("global_state")],
      this.program.programId
    );

    const [providerPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider"), provider.publicKey.toBuffer()],
      this.program.programId
    );

    // Get current service count to determine service ID
    const providerAccount = await this.program.account.provider.fetch(
      providerPda
    );
    const serviceId = providerAccount.serviceCount;

    const [servicePda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("subscription_service"),
        provider.publicKey.toBuffer(),
        serviceId.toBuffer("le", 8),
      ],
      this.program.programId
    );

    const tx = await this.program.methods
      .registerSubscriptionService(
        name,
        description,
        new anchor.BN(feeUsd),
        new anchor.BN(billingDays),
        imageUrl,
        maxSubscribers ? new anchor.BN(maxSubscribers) : null
      )
      .accounts({
        provider: provider.publicKey,
        providerAccount: providerPda,
        globalState: globalStatePda,
        subscriptionService: servicePda,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([provider])
      .rpc();

    console.log(`✅ Service '${name}' created:`, tx);
    return tx;
  }

  /**
   * Get provider information
   */
  async getProvider(providerPubkey: PublicKey): Promise<any> {
    const [providerPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider"), providerPubkey.toBuffer()],
      this.program.programId
    );

    const provider = await this.program.account.provider.fetch(providerPda);
    return {
      wallet: provider.wallet.toString(),
      name: provider.name,
      description: provider.description,
      website: provider.website,
      serviceCount: provider.serviceCount.toNumber(),
      totalSubscribers: provider.totalSubscribers.toNumber(),
      totalRevenue: provider.totalRevenue.toNumber(),
      isVerified: provider.isVerified,
    };
  }

  // ===== USER FUNCTIONS =====

  /**
   * Deposit SOL for subscriptions
   */
  async depositSol(user: Keypair, amount: number): Promise<string> {
    const [globalStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("global_state")],
      this.program.programId
    );

    const [userPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      this.program.programId
    );

    const [solVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), user.publicKey.toBuffer()],
      this.program.programId
    );

    const tx = await this.program.methods
      .deposit(new anchor.BN(amount))
      .accounts({
        user: user.publicKey,
        userAccount: userPda,
        globalState: globalStatePda,
        solVault: solVaultPda,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    console.log(`✅ Deposited ${amount / LAMPORTS_PER_SOL} SOL:`, tx);
    return tx;
  }

  /**
   * Stake SOL for yield generation
   */
  async stakeSol(user: Keypair, amount: number): Promise<string> {
    const [userPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      this.program.programId
    );

    const [stakeAccountPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("stake_account"), user.publicKey.toBuffer()],
      this.program.programId
    );

    const [solVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), user.publicKey.toBuffer()],
      this.program.programId
    );

    const [jitoVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("jito_vault")],
      this.program.programId
    );

    const tx = await this.program.methods
      .stakeSol(new anchor.BN(amount))
      .accounts({
        user: user.publicKey,
        userAccount: userPda,
        stakeAccount: stakeAccountPda,
        solVault: solVaultPda,
        jitoVault: jitoVaultPda,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    console.log(`✅ Staked ${amount / LAMPORTS_PER_SOL} SOL:`, tx);
    return tx;
  }

  /**
   * Subscribe to a service
   */
  async subscribeToService(
    user: Keypair,
    providerPubkey: PublicKey,
    serviceId: number
  ): Promise<string> {
    const [userPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      this.program.programId
    );

    const userAccount = await this.program.account.user.fetch(userPda);

    const [subscriptionPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("user_subscription"),
        user.publicKey.toBuffer(),
        userAccount.subscriptionCount.toBuffer("le", 8),
      ],
      this.program.programId
    );

    const [servicePda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("subscription_service"),
        providerPubkey.toBuffer(),
        new anchor.BN(serviceId).toBuffer("le", 8),
      ],
      this.program.programId
    );

    const [providerPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("provider"), providerPubkey.toBuffer()],
      this.program.programId
    );

    const [globalStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("global_state")],
      this.program.programId
    );

    const certificateNftMint = Keypair.generate();
    const [certificateNftTokenAccount] = PublicKey.findProgramAddressSync(
      [
        user.publicKey.toBuffer(),
        anchor.utils.token.TOKEN_PROGRAM_ID.toBuffer(),
        certificateNftMint.publicKey.toBuffer(),
      ],
      anchor.utils.token.ASSOCIATED_PROGRAM_ID
    );

    const tx = await this.program.methods
      .subscribeToService(providerPubkey, new anchor.BN(serviceId))
      .accounts({
        user: user.publicKey,
        userAccount: userPda,
        subscriptionService: servicePda,
        providerAccount: providerPda,
        userSubscription: subscriptionPda,
        globalState: globalStatePda,
        certificateNftMint: certificateNftMint.publicKey,
        certificateNftTokenAccount: certificateNftTokenAccount,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user, certificateNftMint])
      .rpc();

    console.log(`✅ Subscribed to service:`, tx);
    return tx;
  }

  /**
   * Get user account information
   */
  async getUser(userPubkey: PublicKey): Promise<any> {
    const [userPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user"), userPubkey.toBuffer()],
      this.program.programId
    );

    const user = await this.program.account.user.fetch(userPda);
    return {
      wallet: user.wallet.toString(),
      depositedSol: user.depositedSol.toNumber() / LAMPORTS_PER_SOL,
      lockedSol: user.lockedSol.toNumber() / LAMPORTS_PER_SOL,
      stakedSol: user.stakedSol.toNumber() / LAMPORTS_PER_SOL,
      subscriptionCount: user.subscriptionCount.toNumber(),
      totalPaid: user.totalPaid.toNumber() / LAMPORTS_PER_SOL,
    };
  }

  /**
   * Claim staking yield
   */
  async claimYield(user: Keypair): Promise<string> {
    const [userPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user"), user.publicKey.toBuffer()],
      this.program.programId
    );

    const [stakeAccountPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("stake_account"), user.publicKey.toBuffer()],
      this.program.programId
    );

    const [solVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), user.publicKey.toBuffer()],
      this.program.programId
    );

    const [jitoVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("jito_vault")],
      this.program.programId
    );

    const tx = await this.program.methods
      .claimYield()
      .accounts({
        user: user.publicKey,
        userAccount: userPda,
        stakeAccount: stakeAccountPda,
        solVault: solVaultPda,
        jitoVault: jitoVaultPda,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    console.log(`✅ Claimed yield:`, tx);
    return tx;
  }

  // ===== UTILITY FUNCTIONS =====

  /**
   * Get subscription service information
   */
  async getSubscriptionService(
    providerPubkey: PublicKey,
    serviceId: number
  ): Promise<any> {
    const [servicePda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("subscription_service"),
        providerPubkey.toBuffer(),
        new anchor.BN(serviceId).toBuffer("le", 8),
      ],
      this.program.programId
    );

    const service = await this.program.account.subscriptionService.fetch(
      servicePda
    );
    return {
      provider: service.provider.toString(),
      serviceId: service.serviceId.toNumber(),
      name: service.name,
      description: service.description,
      feeUsd: service.feeUsd.toNumber() / 100, // Convert cents to dollars
      billingFrequencyDays: service.billingFrequencyDays.toNumber(),
      imageUrl: service.imageUrl,
      maxSubscribers: service.maxSubscribers?.toNumber(),
      currentSubscribers: service.currentSubscribers.toNumber(),
      isActive: service.isActive,
    };
  }

  /**
   * Calculate required stake for subscriptions
   */
  calculateRequiredStake(
    monthlyFeeUsd: number,
    months: number = 12,
    apy: number = 0.05,
    solPriceUsd: number = 100
  ): number {
    const totalFeeUsd = monthlyFeeUsd * months;
    const requiredYieldUsd = totalFeeUsd;
    const requiredStakeUsd = requiredYieldUsd / apy;
    const requiredStakeSol = requiredStakeUsd / solPriceUsd;
    return requiredStakeSol * LAMPORTS_PER_SOL;
  }
}

// Example usage
export async function exampleUsage() {
  // Initialize integration
  const subly = new SublyIntegration(
    "5DpoKLMkQSBTi3n6hnjB7RPhzjhovfDZbEHJvFJBXKL9", // Program ID
    "https://api.devnet.solana.com", // RPC URL
    {} // Wallet (replace with actual wallet)
  );

  // Example: Provider workflow
  const provider = Keypair.generate();
  await subly.registerProvider(
    provider,
    "StreamFlix",
    "Premium video streaming service",
    "https://streamflix.com"
  );

  await subly.createSubscriptionService(
    provider,
    "StreamFlix Premium",
    "4K streaming, multiple devices",
    1299, // $12.99
    30, // 30 days
    "https://streamflix.com/logo.png",
    100000 // max subscribers
  );

  // Example: User workflow
  const user = Keypair.generate();

  // Deposit and stake SOL
  await subly.depositSol(user, 5 * LAMPORTS_PER_SOL);
  await subly.stakeSol(user, 3 * LAMPORTS_PER_SOL);

  // Subscribe to service
  await subly.subscribeToService(user, provider.publicKey, 0);

  // Check user info
  const userInfo = await subly.getUser(user.publicKey);
  console.log("User info:", userInfo);

  // Claim yield periodically
  setTimeout(async () => {
    await subly.claimYield(user);
  }, 24 * 60 * 60 * 1000); // After 24 hours
}

export default SublyIntegration;
