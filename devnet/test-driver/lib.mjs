// Shared helpers for Memecoin.io test driver.

import {
  Connection,
  Keypair,
  PublicKey,
} from "@solana/web3.js";
import fs from "node:fs";
import crypto from "node:crypto";

import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, "..", "..");

export const RPC = process.env.MEMECOIN_RPC || "http://127.0.0.1:8899";
export const KEYS = process.env.MEMECOIN_KEYS_DIR || join(REPO_ROOT, "devnet", "keys");
export const PROGRAM_ID = new PublicKey("CuJx7BY4Zu9tJzbDTGGTm8tMpE1v2PTEeecB5qfCmhs3");
export const METAPLEX_PROGRAM_ID = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
export const STATE_FILE = process.env.MEMECOIN_STATE_FILE || join(__dirname, "state.json");

export function loadKp(name) {
  const raw = JSON.parse(fs.readFileSync(`${KEYS}/${name}.json`, "utf8"));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

export function disc(name) {
  return crypto.createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}

export function loadState() {
  return JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
}

export function saveState(s) {
  fs.writeFileSync(STATE_FILE, JSON.stringify(s, null, 2));
}

export function pdaConfig() {
  return PublicKey.findProgramAddressSync([Buffer.from("config")], PROGRAM_ID);
}

export function pdaLaunch(mint) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("launch"), mint.toBuffer()],
    PROGRAM_ID,
  );
}

export function pdaVault(mint) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), mint.toBuffer()],
    PROGRAM_ID,
  );
}

export function pdaMintAuthority(mint) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("mint_authority"), mint.toBuffer()],
    PROGRAM_ID,
  );
}

export function pdaMetadata(mint) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("metadata"), METAPLEX_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    METAPLEX_PROGRAM_ID,
  );
}

// Bonding curve (matches utils.rs sol_in_for_tokens_out):
//   x = virtual_sol + real_sol;  y = virtual_tokens (constant)
//   k = x * y
//   new_y = y - tokens_out
//   new_x = ceil(k / new_y)
//   sol_in = new_x - x
export function quoteSolIn(vSol, vTok, rSol, _rTok, dy) {
  const x = BigInt(vSol) + BigInt(rSol);
  const y = BigInt(vTok);
  const k = x * y;
  const new_y = y - BigInt(dy);
  if (new_y <= 0n) throw new Error("token_amount_out exceeds curve");
  // ceil(k / new_y) = (k + new_y - 1) / new_y
  const new_x = (k + new_y - 1n) / new_y;
  return new_x - x;
}

// tokens_out for sol_in (forward direction)
export function quoteTokensOut(vSol, vTok, rSol, solIn) {
  const x = BigInt(vSol) + BigInt(rSol);
  const y = BigInt(vTok);
  const k = x * y;
  const new_x = x + BigInt(solIn);
  const new_y = k / new_x; // floor
  return y - new_y;
}

export const connection = new Connection(RPC, "confirmed");
