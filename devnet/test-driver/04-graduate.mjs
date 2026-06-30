// Graduate the bonding curve.
// Permissionless crank — anyone signs as `caller`; we use buyer1.
// Account order (must match graduate.rs):
//   caller (signer, mut)
//   config
//   launch (mut)
//   mint (mut)
//   curve_vault (mut)
//   mint_authority
//   graduation_authority (mut, NOT a signer)
//   graduation_authority_token_account (mut, ATA — init_if_needed)
//   dino_buyback_wallet (mut)
//   treasury_wallet (mut)
//   system_program
//   token_program
//   associated_token_program
//   rent

import {
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress,
} from "@solana/spl-token";
import {
  connection,
  loadKp,
  loadState,
  saveState,
  disc,
  PROGRAM_ID,
  pdaConfig,
  pdaLaunch,
  pdaVault,
  pdaMintAuthority,
} from "./lib.mjs";

const state = loadState();
const caller = loadKp("buyer1");
const mint = new PublicKey(state.mint);
const [configPda] = pdaConfig();
const [launchPda] = pdaLaunch(mint);
const [vaultPda] = pdaVault(mint);
const [mintAuthPda] = pdaMintAuthority(mint);

const gradAuth = new PublicKey("DGvnDqpsb5CJyCenFbtUgf9Xg3XpfqjAodnrtqd8fwwv");
const dinoBuyback = new PublicKey("GFLvHvSsx88V89YVTgYpn3jP5hUxeyxrVwNRvdwy1kna");
const treasury = new PublicKey("GkLTWAeWYespURD54EgWRdG6WXco3rMr9MW3cmAQsgp9");

const gradAuthAta = await getAssociatedTokenAddress(mint, gradAuth);
console.log("Caller:", caller.publicKey.toBase58());
console.log("Mint:", mint.toBase58());
console.log("Launch:", launchPda.toBase58());
console.log("Vault:", vaultPda.toBase58());
console.log("MintAuth:", mintAuthPda.toBase58());
console.log("GradAuth ATA:", gradAuthAta.toBase58());

// Pre-balances
const preDino = await connection.getBalance(dinoBuyback);
const preTreasury = await connection.getBalance(treasury);
const preGradAuth = await connection.getBalance(gradAuth);
const preVault = await connection.getBalance(vaultPda);
console.log(`Pre: vault=${preVault} dino=${preDino} treasury=${preTreasury} gradAuth=${preGradAuth}`);

const data = disc("graduate"); // no args

const keys = [
  { pubkey: caller.publicKey, isSigner: true, isWritable: true },
  { pubkey: configPda, isSigner: false, isWritable: false },
  { pubkey: launchPda, isSigner: false, isWritable: true },
  { pubkey: mint, isSigner: false, isWritable: true },
  { pubkey: vaultPda, isSigner: false, isWritable: true },
  { pubkey: mintAuthPda, isSigner: false, isWritable: false },
  { pubkey: gradAuth, isSigner: false, isWritable: true },
  { pubkey: gradAuthAta, isSigner: false, isWritable: true },
  { pubkey: dinoBuyback, isSigner: false, isWritable: true },
  { pubkey: treasury, isSigner: false, isWritable: true },
  { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
  { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
  { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
];

const ix = new TransactionInstruction({ programId: PROGRAM_ID, keys, data });
// Give CU room for ATA init + mint_to + lamport debits/credits
const cu = ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 });

const tx = new Transaction().add(cu, ix);
const sig = await connection.sendTransaction(tx, [caller], { skipPreflight: false });
await connection.confirmTransaction(sig, "confirmed");
console.log("✓ graduate TX:", sig);

// Post-balances
const postDino = await connection.getBalance(dinoBuyback);
const postTreasury = await connection.getBalance(treasury);
const postGradAuth = await connection.getBalance(gradAuth);
const postVault = await connection.getBalance(vaultPda);
console.log(`Post: vault=${postVault} dino=${postDino} treasury=${postTreasury} gradAuth=${postGradAuth}`);
console.log(`Delta: dino=${postDino - preDino} treasury=${postTreasury - preTreasury} gradAuth=${postGradAuth - preGradAuth}`);

// Verify mint reserve hit graduation authority ATA
const ataInfo = await connection.getParsedAccountInfo(gradAuthAta);
const tokenAmount = ataInfo.value?.data?.parsed?.info?.tokenAmount?.amount;
console.log(`GradAuth ATA token amount: ${tokenAmount} (should be 200,000,000 * 10^6 = 200_000_000_000_000)`);

state.graduate_sig = sig;
state.graduate_ata = gradAuthAta.toBase58();
saveState(state);
