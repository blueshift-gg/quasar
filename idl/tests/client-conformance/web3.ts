import { Address } from "@solana/web3.js";
import {
  GoldenDemoClient,
  PROGRAM_ERRORS,
  ProgramEvent,
  VaultCodec,
  findVaultAddress,
} from "./client.js";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

async function main() {
  const authority = new Address("11111111111111111111111111111111");
  const client = new GoldenDemoClient();
  const expectedVault = await findVaultAddress(authority);
  const instruction = await client.createMakeInstruction({
    authority,
    amount: 42n,
    flag: true,
  });

  assert(instruction.keys.length === 2, "instruction account count changed");
  assert(instruction.keys[0].isSigner, "authority signer flag changed");
  assert(!instruction.keys[0].isWritable, "authority writable flag changed");
  assert(
    instruction.keys[1].pubkey.toString() === expectedVault.toString(),
    "PDA account changed",
  );
  assert(instruction.keys[1].isWritable, "vault role changed");

  const decodedInstruction = client.decodeInstruction(instruction.data);
  assert(decodedInstruction?.type === "Make", "instruction decoder failed");
  assert(decodedInstruction.args.amount === 42n, "amount encoding changed");
  assert(decodedInstruction.args.flag, "boolean encoding changed");

  const encodedVault = VaultCodec.encode({
    authority,
    amount: 42n,
    mode: 1,
  });
  const decodedVault = client.decodeVault(
    new Uint8Array([42, ...encodedVault]),
  );
  assert(
    decodedVault.authority.toString() === authority.toString(),
    "account address changed",
  );
  assert(decodedVault.amount === 42n, "account amount changed");
  assert(decodedVault.mode === 1, "account mode changed");

  assert(
    client.decodeEvent(new Uint8Array([7]))?.type === ProgramEvent.VaultMade,
    "event decoder failed",
  );
  assert(
    client.decodeEvent(new Uint8Array([99])) === null,
    "unknown event discriminator was accepted",
  );
  assert(PROGRAM_ERRORS[6000]?.name === "Unauthorized", "error map changed");
}

void main();
