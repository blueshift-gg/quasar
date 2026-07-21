import { AccountRole, address } from "@solana/kit";
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
  const authority = address("11111111111111111111111111111111");
  const client = new GoldenDemoClient();
  const expectedVault = await findVaultAddress(authority);
  const instruction = await client.createMakeInstruction({
    authority,
    amount: 42n,
    flag: true,
  });
  const accounts = instruction.accounts as readonly {
    address: string;
    role: unknown;
  }[];

  assert(accounts.length === 2, "instruction account count changed");
  assert(
    accounts[0].role === AccountRole.READONLY_SIGNER,
    "authority role changed",
  );
  assert(accounts[1].address === expectedVault, "PDA account changed");
  assert(accounts[1].role === AccountRole.WRITABLE, "vault role changed");

  const instructionData = instruction.data;
  assert(instructionData !== undefined, "instruction data is missing");
  const decodedInstruction = client.decodeInstruction(instructionData);
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
  assert(decodedVault.authority === authority, "account address changed");
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
