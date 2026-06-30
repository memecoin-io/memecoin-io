// Seal graduation by recording a mock Meteora DLMM pool pubkey.
// Signed by graduation_authority.
//
// Account order:
//   graduation_authority (signer, mut)
//   config
//   launch (mut)
//
// Args: meteora_lb_pair: Pubkey (32 bytes)

import {
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  connection,
  loadKp,
  loadState,
  saveState,
  disc,
  PROGRAM_ID,
  pdaConfig,
  pdaLaunch,
} from "./lib.mjs";

const state = loadState();
const gradAuth = loadKp("graduation_authority");
const mint = new PublicKey(state.mint);
const [configPda] = pdaConfig();
const [launchPda] = pdaLaunch(mint);

// Mock LB pair — random pubkey representing the Meteora DLMM pool
const mockLbPair = Keypair.generate().publicKey;
console.log("Mock Meteora LB pair:", mockLbPair.toBase58());

// Build instruction data: disc + 32-byte pubkey
const data = Buffer.concat([disc("seal_graduation"), mockLbPair.toBuffer()]);

const keys = [
  { pubkey: gradAuth.publicKey, isSigner: true, isWritable: true },
  { pubkey: configPda, isSigner: false, isWritable: false },
  { pubkey: launchPda, isSigner: false, isWritable: true },
];

const ix = new TransactionInstruction({ programId: PROGRAM_ID, keys, data });
const tx = new Transaction().add(ix);
const sig = await connection.sendTransaction(tx, [gradAuth], { skipPreflight: false });
await connection.confirmTransaction(sig, "confirmed");
console.log("✓ seal_graduation TX:", sig);

// Verify launch is marked sealed
const acct = await connection.getAccountInfo(launchPda);
console.log("Launch account size:", acct.data.length);

state.seal_sig = sig;
state.mock_lb_pair = mockLbPair.toBase58();
saveState(state);
