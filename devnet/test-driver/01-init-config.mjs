// Initialize the global config PDA on the deployed Memecoin program.
// Uses manual Anchor instruction encoding (no IDL needed).

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import fs from "node:fs";
import crypto from "node:crypto";
import BN from "bn.js";

const RPC = "http://127.0.0.1:8899";
const KEYS = "/home/user/workspace/memecoin-io/devnet/keys";
const PROGRAM_ID = new PublicKey("CuJx7BY4Zu9tJzbDTGGTm8tMpE1v2PTEeecB5qfCmhs3");

function loadKp(name) {
  const raw = JSON.parse(fs.readFileSync(`${KEYS}/${name}.json`, "utf8"));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

// Anchor instruction discriminator = sha256("global:<name>")[..8]
function disc(name) {
  return crypto.createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}

// Serializer for InitializeConfigParams (matches borsh order in source)
function encodeInitParams(p) {
  const buf = Buffer.alloc(
    32 * 3 +              // dino_buyback_wallet, treasury_wallet, graduation_authority
    2 + 2 + 2 + 2 +       // trade_fee_bps + 3 split bps
    8 + 2 +               // creation_fee_lamports + split
    8 + 2 +               // graduation_fee_lamports + split
    8                     // graduation_threshold_lamports
  );
  let off = 0;
  p.dino_buyback_wallet.toBuffer().copy(buf, off); off += 32;
  p.treasury_wallet.toBuffer().copy(buf, off); off += 32;
  p.graduation_authority.toBuffer().copy(buf, off); off += 32;
  buf.writeUInt16LE(p.trade_fee_bps, off); off += 2;
  buf.writeUInt16LE(p.trade_split_creator_bps, off); off += 2;
  buf.writeUInt16LE(p.trade_split_dino_bps, off); off += 2;
  buf.writeUInt16LE(p.trade_split_treasury_bps, off); off += 2;
  buf.writeBigUInt64LE(BigInt(p.creation_fee_lamports), off); off += 8;
  buf.writeUInt16LE(p.creation_split_dino_bps, off); off += 2;
  buf.writeBigUInt64LE(BigInt(p.graduation_fee_lamports), off); off += 8;
  buf.writeUInt16LE(p.graduation_split_dino_bps, off); off += 2;
  buf.writeBigUInt64LE(BigInt(p.graduation_threshold_lamports), off); off += 8;
  return buf;
}

const connection = new Connection(RPC, "confirmed");

const admin = loadKp("deployer");
const dinoBuyback = loadKp("dino_buyback");
const treasury = loadKp("treasury");
const gradAuth = loadKp("graduation_authority");

console.log("Admin:", admin.publicKey.toBase58());
console.log("DINO buyback:", dinoBuyback.publicKey.toBase58());
console.log("Treasury:", treasury.publicKey.toBase58());
console.log("Grad authority:", gradAuth.publicKey.toBase58());

// Derive config PDA
const [configPda, configBump] = PublicKey.findProgramAddressSync(
  [Buffer.from("config")],
  PROGRAM_ID,
);
console.log("Config PDA:", configPda.toBase58(), "bump:", configBump);

const params = {
  dino_buyback_wallet: dinoBuyback.publicKey,
  treasury_wallet: treasury.publicKey,
  graduation_authority: gradAuth.publicKey,
  trade_fee_bps: 100,
  trade_split_creator_bps: 3333,
  trade_split_dino_bps: 3333,
  trade_split_treasury_bps: 3334,
  creation_fee_lamports: 20_000_000,
  creation_split_dino_bps: 5000,
  graduation_fee_lamports: 1_000_000_000,
  graduation_split_dino_bps: 5000,
  graduation_threshold_lamports: 5 * 1_000_000_000, // beta cap: 5 SOL
};

const data = Buffer.concat([disc("initialize_config"), encodeInitParams(params)]);

const ix = new TransactionInstruction({
  programId: PROGRAM_ID,
  keys: [
    { pubkey: admin.publicKey, isSigner: true, isWritable: true },
    { pubkey: configPda, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ],
  data,
});

const tx = new Transaction().add(ix);
const sig = await connection.sendTransaction(tx, [admin], { skipPreflight: false });
console.log("TX signature:", sig);
await connection.confirmTransaction(sig, "confirmed");

// Read back config to verify
const acct = await connection.getAccountInfo(configPda);
console.log("Config account size:", acct.data.length, "lamports:", acct.lamports);
console.log("Owner:", acct.owner.toBase58());

// Save state to file for next steps
fs.writeFileSync(
  "/home/user/workspace/memecoin-io/devnet/test-driver/state.json",
  JSON.stringify(
    {
      program_id: PROGRAM_ID.toBase58(),
      config_pda: configPda.toBase58(),
      config_bump: configBump,
      init_config_sig: sig,
      params: {
        ...params,
        dino_buyback_wallet: params.dino_buyback_wallet.toBase58(),
        treasury_wallet: params.treasury_wallet.toBase58(),
        graduation_authority: params.graduation_authority.toBase58(),
      },
    },
    null,
    2,
  ),
);
console.log("✓ initialize_config done");
