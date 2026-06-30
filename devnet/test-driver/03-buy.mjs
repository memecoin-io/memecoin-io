// buy: drive buys from buyer1..buyer5 until graduation threshold (55 SOL real_sol) is reached.

import {
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";
import BN from "bn.js";
import {
  connection,
  loadKp,
  disc,
  PROGRAM_ID,
  pdaConfig,
  loadState,
  saveState,
  quoteSolIn,
} from "./lib.mjs";

const state = loadState();
const mint = new PublicKey(state.mint);
const launchPda = new PublicKey(state.launch_pda);
const vaultPda = new PublicKey(state.vault_pda);
const curveAta = new PublicKey(state.curve_ata);
const creator = new PublicKey("AVoyA3zPmBSkR989XefTj3PDP1qzNum88qzqqkd32rLN");
const dinoBuyback = new PublicKey(state.params.dino_buyback_wallet);
const treasury = new PublicKey(state.params.treasury_wallet);
const [configPda] = pdaConfig();

const THRESHOLD = 5_000_000_000n; // 5 SOL (beta cap)

async function getCurveState() {
  const acct = await connection.getAccountInfo(launchPda);
  const raw = acct.data;
  let off = 8 + 32 + 32 + 8;
  const real_sol = raw.readBigUInt64LE(off); off += 8;
  const real_tok = raw.readBigUInt64LE(off); off += 8;
  const virt_sol = raw.readBigUInt64LE(off); off += 8;
  const virt_tok = raw.readBigUInt64LE(off); off += 8;
  return { real_sol, real_tok, virt_sol, virt_tok };
}

async function ensureBuyerAta(buyer) {
  const ata = getAssociatedTokenAddressSync(mint, buyer.publicKey);
  const info = await connection.getAccountInfo(ata);
  if (!info) {
    const ix = createAssociatedTokenAccountInstruction(
      buyer.publicKey, ata, buyer.publicKey, mint,
    );
    const tx = new Transaction().add(ix);
    const sig = await connection.sendTransaction(tx, [buyer], { skipPreflight: false });
    await connection.confirmTransaction(sig, "confirmed");
  }
  return ata;
}

async function doBuy(buyerName, tokensOut) {
  const buyer = loadKp(buyerName);
  const { real_sol, real_tok, virt_sol, virt_tok } = await getCurveState();

  // Quote SOL needed for tokens_out
  const solToCurve = quoteSolIn(virt_sol, virt_tok, real_sol, real_tok, tokensOut);
  const fee = solToCurve / 100n; // 1%
  const maxSolIn = solToCurve + fee + 10_000_000n; // +0.01 SOL slack

  console.log(`  ${buyerName}: tokens_out=${tokensOut} → curve_sol=${Number(solToCurve)/1e9} SOL, max=${Number(maxSolIn)/1e9} SOL`);

  const buyerAta = await ensureBuyerAta(buyer);

  const data = Buffer.alloc(24);
  disc("buy").copy(data, 0);
  Buffer.from(new BN(tokensOut.toString()).toArray('le', 8)).copy(data, 8);
  Buffer.from(new BN(maxSolIn.toString()).toArray('le', 8)).copy(data, 16);

  const ix = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: buyer.publicKey, isSigner: true, isWritable: true },
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: launchPda, isSigner: false, isWritable: true },
      { pubkey: mint, isSigner: false, isWritable: false },
      { pubkey: vaultPda, isSigner: false, isWritable: true },
      { pubkey: curveAta, isSigner: false, isWritable: true },
      { pubkey: buyerAta, isSigner: false, isWritable: true },
      { pubkey: creator, isSigner: false, isWritable: true },
      { pubkey: dinoBuyback, isSigner: false, isWritable: true },
      { pubkey: treasury, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    ],
    data,
  });

  const tx = new Transaction().add(ix);
  try {
    const sig = await connection.sendTransaction(tx, [buyer], { skipPreflight: false });
    await connection.confirmTransaction(sig, "confirmed");
    return sig;
  } catch (e) {
    console.error("  ✗ buy failed:", e.message);
    if (e.transactionLogs) console.error("  logs:", e.transactionLogs.slice(-3));
    throw e;
  }
}

// Strategy: each iteration check real_sol vs threshold. Buy a chunk that lifts us a bit.
// Use buyers in rotation. Buy in chunks of ~5-10% of remaining curve tokens.

const buyers = ["buyer1", "buyer2", "buyer3", "buyer4", "buyer5"];
const txSigs = [];
let idx = 0;

// Strategy for 5 SOL cap: solve curve math to land just above threshold.
// At threshold real_sol=5 SOL, virt_sol=30 SOL, virt_tok=1.236e15:
//   x_end = 35e9, y_end = k / x_end where k = 30e9 * 1.236e15
//   tokens to sell to curve = virt_tok - y_end ≈ 1.766e14 ≈ 176B base units
// We target small chunks (~20B base units = ~10% of needed) so we step gently.
const CHUNK = 20_000_000_000_000n; // 20B base units = 20M tokens

for (let iter = 0; iter < 100; iter++) {
  const { real_sol, real_tok, virt_sol, virt_tok } = await getCurveState();
  console.log(`\n=== iter ${iter}: real_sol=${Number(real_sol)/1e9} SOL, real_tok=${real_tok} ===`);
  if (real_sol >= THRESHOLD) {
    console.log("✓ Threshold reached");
    break;
  }
  if (real_tok <= 1_000_000_000n) {
    console.log("Curve nearly empty");
    break;
  }

  // Compute max tokens we can buy without breaching the 5 SOL cap.
  // If sol_in for CHUNK would push us past threshold, shrink to land just below threshold + a tiny bit over.
  let chunk = CHUNK;
  // Aim for AT MOST threshold (5 SOL exactly). JS quoteSolIn matches on-chain math (both use floor div).
  const MAX_TARGET = THRESHOLD;
  const projSol = quoteSolIn(virt_sol, virt_tok, real_sol, real_tok, chunk);
  if (real_sol + projSol > MAX_TARGET) {
    // shrink: binary-search largest chunk where post-buy real_sol <= MAX_TARGET
    let lo = 1_000_000n, hi = chunk;
    while (lo < hi) {
      const mid = (lo + hi + 1n) / 2n;
      const s = quoteSolIn(virt_sol, virt_tok, real_sol, real_tok, mid);
      if (real_sol + s <= MAX_TARGET) lo = mid;
      else hi = mid - 1n;
    }
    chunk = lo;
  }
  if (chunk > real_tok) chunk = real_tok;
  if (chunk < 500_000n) {
    console.log("Chunk too small (<0.5M tokens), threshold effectively reached");
    break;
  }

  const buyerName = buyers[idx % buyers.length];
  idx++;
  const sig = await doBuy(buyerName, chunk);
  txSigs.push({ iter, buyer: buyerName, sig });
  console.log(`  ✓ TX ${sig.slice(0, 20)}...`);
}

const final = await getCurveState();
console.log("\n=== Final ===");
console.log(`real_sol: ${Number(final.real_sol) / 1e9} SOL (threshold ${Number(THRESHOLD)/1e9})`);
console.log(`real_tok: ${final.real_tok}`);
console.log(`total buys: ${txSigs.length}`);

state.buy_sigs = txSigs;
state.real_sol_after_buys = final.real_sol.toString();
state.real_tok_after_buys = final.real_tok.toString();
saveState(state);
console.log("✓ buys complete");
