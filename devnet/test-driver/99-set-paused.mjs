import { PublicKey, Transaction, TransactionInstruction } from "@solana/web3.js";
import { connection, loadKp, disc, PROGRAM_ID, pdaConfig } from "./lib.mjs";

const admin = loadKp("deployer");
const [configPda] = pdaConfig();

// Try setting paused=false explicitly
const data = Buffer.concat([disc("set_paused"), Buffer.from([0])]);

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
console.log("set_paused=false TX:", sig);
