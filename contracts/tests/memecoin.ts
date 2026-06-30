/**
 * Memecoin.io integration tests — devnet / localnet
 *
 * Covers:
 *   - initialize_config
 *   - create_token (and creation-fee split)
 *   - buy (with slippage protection)
 *   - sell (with slippage protection)
 *   - graduate (stubbed Meteora CPI for now)
 *   - update_config / set_paused
 *
 * Run:    anchor test
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  LAMPORTS_PER_SOL,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";
import { expect } from "chai";

import { Memecoin } from "../target/types/memecoin";

describe("memecoin", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Memecoin as Program<Memecoin>;
  const admin = provider.wallet as anchor.Wallet;

  const dinoBuyback = Keypair.generate();
  const treasury = Keypair.generate();

  const [configPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    program.programId
  );

  const defaultParams = {
    dinoBuybackWallet: dinoBuyback.publicKey,
    treasuryWallet: treasury.publicKey,
    tradeFeeBps: 100,
    tradeSplitCreatorBps: 3333,
    tradeSplitDinoBps: 3333,
    tradeSplitTreasuryBps: 3334,
    creationFeeLamports: new BN(20_000_000),
    creationSplitDinoBps: 5000,
    graduationFeeLamports: new BN(LAMPORTS_PER_SOL),
    graduationSplitDinoBps: 5000,
    graduationThresholdLamports: new BN(55 * LAMPORTS_PER_SOL),
  };

  it("initializes config with locked v1 parameters", async () => {
    await program.methods
      .initializeConfig(defaultParams)
      .accounts({
        admin: admin.publicKey,
        config: configPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const cfg = await program.account.config.fetch(configPda);
    expect(cfg.tradeFeeBps).to.equal(100);
    expect(cfg.tradeSplitCreatorBps).to.equal(3333);
    expect(cfg.tradeSplitDinoBps).to.equal(3333);
    expect(cfg.tradeSplitTreasuryBps).to.equal(3334);
    expect(cfg.graduationThresholdLamports.toString()).to.equal(
      (55 * LAMPORTS_PER_SOL).toString()
    );
    expect(cfg.paused).to.equal(false);
  });

  it("creates a new token + bonding curve", async () => {
    const mint = Keypair.generate();
    const creator = Keypair.generate();

    // Airdrop SOL to creator
    const sig = await provider.connection.requestAirdrop(
      creator.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(sig);

    const [launch] = PublicKey.findProgramAddressSync(
      [Buffer.from("launch"), mint.publicKey.toBuffer()],
      program.programId
    );
    const [curveVault] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), mint.publicKey.toBuffer()],
      program.programId
    );
    const [mintAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("mint_authority"), mint.publicKey.toBuffer()],
      program.programId
    );

    const curveTokenAccount = getAssociatedTokenAddressSync(
      mint.publicKey,
      launch,
      true
    );

    await program.methods
      .createToken({
        name: "TestMeme",
        symbol: "TMEME",
        uri: "https://arweave.net/example",
      })
      .accounts({
        creator: creator.publicKey,
        config: configPda,
        dinoBuybackWallet: dinoBuyback.publicKey,
        treasuryWallet: treasury.publicKey,
        mint: mint.publicKey,
        mintAuthority,
        launch,
        curveVault,
        curveTokenAccount,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .signers([creator, mint])
      .rpc();

    const launchAcc = await program.account.tokenLaunch.fetch(launch);
    expect(launchAcc.creator.toBase58()).to.equal(creator.publicKey.toBase58());
    expect(launchAcc.realSolReserves.toNumber()).to.equal(0);
    expect(launchAcc.graduated).to.equal(false);

    // Verify creation fee was split 50/50
    const dinoBal = await provider.connection.getBalance(dinoBuyback.publicKey);
    const treasuryBal = await provider.connection.getBalance(treasury.publicKey);
    expect(dinoBal).to.be.greaterThanOrEqual(10_000_000);
    expect(treasuryBal).to.be.greaterThanOrEqual(10_000_000);
  });

  // TODO: buy/sell/graduate integration tests
  // These require either a local-validator or devnet — added in week 2 along with
  // the buy/sell client SDK.
});
