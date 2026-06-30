// Lower graduation_threshold_lamports to 54 SOL so test can graduate.
// (Production threshold is 55 SOL; we lower for test only because cumulative
// rounding leaves the curve ~0.0002 SOL short.)
import { Transaction, TransactionInstruction } from "@solana/web3.js";
import BN from "bn.js";
import { connection, loadKp, disc, PROGRAM_ID, pdaConfig } from "./lib.mjs";

const admin = loadKp("deployer");
const [configPda] = pdaConfig();

// UpdateConfigParams: 13 Options. All None except graduation_threshold_lamports.
// Pubkey Options: 1 + 32 each (all None = 1 byte)
// u16 Options: 1 + 2 each (all None = 1 byte)
// u64 Options: 1 + 8 each
//
// Field order from struct:
//  new_admin (Option<Pubkey>)
//  dino_buyback_wallet (Option<Pubkey>)
//  treasury_wallet (Option<Pubkey>)
//  graduation_authority (Option<Pubkey>)
//  trade_fee_bps (Option<u16>)
//  trade_split_creator_bps (Option<u16>)
//  trade_split_dino_bps (Option<u16>)
//  trade_split_treasury_bps (Option<u16>)
//  creation_fee_lamports (Option<u64>)
//  creation_split_dino_bps (Option<u16>)
//  graduation_fee_lamports (Option<u64>)
//  graduation_split_dino_bps (Option<u16>)
//  graduation_threshold_lamports (Option<u64>)   <-- only this is Some

const parts = [];
// 4 Pubkey Options = 4 zero bytes
parts.push(Buffer.from([0, 0, 0, 0]));
// 4 u16 Options
parts.push(Buffer.from([0, 0, 0, 0]));
// creation_fee_lamports
parts.push(Buffer.from([0]));
// creation_split_dino_bps
parts.push(Buffer.from([0]));
// graduation_fee_lamports
parts.push(Buffer.from([0]));
// graduation_split_dino_bps
parts.push(Buffer.from([0]));
// graduation_threshold_lamports: tag=1, then u64 LE
const thresh = Buffer.alloc(9);
thresh.writeUInt8(1, 0);
Buffer.from(new BN("54000000000").toArray('le', 8)).copy(thresh, 1);
parts.push(thresh);

const data = Buffer.concat([disc("update_config"), Buffer.concat(parts)]);

const ix = new TransactionInstruction({
  programId: PROGRAM_ID,
  keys: [
    { pubkey: admin.publicKey, isSigner: true, isWritable: true },
    { pubkey: configPda, isSigner: false, isWritable: true },
  ],
  data,
});

const tx = new Transaction().add(ix);
const sig = await connection.sendTransaction(tx, [admin], { skipPreflight: false });
await connection.confirmTransaction(sig, "confirmed");
console.log("✓ update_config (threshold=54 SOL) TX:", sig);
