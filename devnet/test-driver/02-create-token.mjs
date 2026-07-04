// create_token: create new mint + bonding curve, pay creation fee.

import {
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import {
  connection,
  loadKp,
  disc,
  PROGRAM_ID,
  METAPLEX_PROGRAM_ID,
  pdaConfig,
  pdaLaunch,
  pdaVault,
  pdaMintAuthority,
  pdaMetadata,
  loadState,
  saveState,
} from "./lib.mjs";

function encStr(s) {
  const utf = Buffer.from(s, "utf8");
  const buf = Buffer.alloc(4 + utf.length);
  buf.writeUInt32LE(utf.length, 0);
  utf.copy(buf, 4);
  return buf;
}
function encodeCreateParams(name, symbol, uri) {
  return Buffer.concat([encStr(name), encStr(symbol), encStr(uri)]);
}

const state = loadState();
const creator = loadKp("creator");
const dinoBuyback = new PublicKey(state.params.dino_buyback_wallet);
const treasury = new PublicKey(state.params.treasury_wallet);

const mintKp = Keypair.generate();
console.log("New mint:", mintKp.publicKey.toBase58());

const [configPda] = pdaConfig();
const [launchPda] = pdaLaunch(mintKp.publicKey);
const [vaultPda] = pdaVault(mintKp.publicKey);
const [mintAuthPda] = pdaMintAuthority(mintKp.publicKey);
const [metadataPda] = pdaMetadata(mintKp.publicKey);

const curveAta = getAssociatedTokenAddressSync(mintKp.publicKey, launchPda, true);

console.log("Launch PDA:", launchPda.toBase58());
console.log("Vault PDA:", vaultPda.toBase58());
console.log("Mint authority PDA:", mintAuthPda.toBase58());
console.log("Metadata PDA:", metadataPda.toBase58());
console.log("Curve ATA:", curveAta.toBase58());

const data = Buffer.concat([
  disc("create_token"),
  encodeCreateParams("Devnet Deni", "DDENI", "https://example.com/ddeni.json"),
]);

const ix = new TransactionInstruction({
  programId: PROGRAM_ID,
  keys: [
    { pubkey: creator.publicKey, isSigner: true, isWritable: true },
    { pubkey: configPda, isSigner: false, isWritable: false },
    { pubkey: dinoBuyback, isSigner: false, isWritable: true },
    { pubkey: treasury, isSigner: false, isWritable: true },
    { pubkey: mintKp.publicKey, isSigner: true, isWritable: true },
    { pubkey: mintAuthPda, isSigner: false, isWritable: false },
    { pubkey: launchPda, isSigner: false, isWritable: true },
    { pubkey: vaultPda, isSigner: false, isWritable: true },
    { pubkey: curveAta, isSigner: false, isWritable: true },
    { pubkey: metadataPda, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: METAPLEX_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
  ],
  data,
});

const tx = new Transaction().add(ix);
const sig = await connection.sendTransaction(tx, [creator, mintKp], { skipPreflight: false });
console.log("TX:", sig);
await connection.confirmTransaction(sig, "confirmed");

const launchAcct = await connection.getAccountInfo(launchPda);
console.log("Launch account size:", launchAcct.data.length, "lamports:", launchAcct.lamports);

state.mint = mintKp.publicKey.toBase58();
state.mint_secret = Array.from(mintKp.secretKey);
state.launch_pda = launchPda.toBase58();
state.vault_pda = vaultPda.toBase58();
state.mint_auth_pda = mintAuthPda.toBase58();
state.metadata_pda = metadataPda.toBase58();
state.curve_ata = curveAta.toBase58();
state.create_token_sig = sig;
saveState(state);

console.log("✓ create_token done");
