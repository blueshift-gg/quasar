import { type Address, address, AccountRole, type Instruction, getProgramDerivedAddress, getAddressCodec, type ClientWithPayer, type ClientWithTransactionPlanning, type ClientWithTransactionSending } from "@solana/kit";
import { addSelfPlanAndSendFunctions } from "@solana/kit/program-client-core";
import { getStructCodec, getU64Codec } from "@solana/codecs";

function matchDisc(data: Uint8Array, disc: Uint8Array): boolean {
  if (data.length < disc.length) return false;
  for (let i = 0; i < disc.length; i++) {
    if (data[i] !== disc[i]) return false;
  }
  return true;
}

const MAX_DECODE_ELEMENTS = 10 * 1024 * 1024;

function checkedLength(value: number | bigint): number {
  const result = Number(value);
  if (!Number.isSafeInteger(result) || result < 0) throw new Error("invalid length prefix");
  return result;
}

function checkedElementCount(value: number | bigint, remaining: number): number {
  const result = checkedLength(value);
  if (result > MAX_DECODE_ELEMENTS || result > remaining) {
    throw new Error("element count exceeds limit");
  }
  return result;
}

function checkedTake(data: Uint8Array, offset: number, size: number): Uint8Array {
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(size) || offset < 0 || size < 0 || size > data.length - offset) {
    throw new Error("truncated input");
  }
  return data.slice(offset, offset + size);
}

function decodeUtf8(data: Uint8Array): string {
  return new TextDecoder("utf-8", { fatal: true }).decode(data);
}

function unwrapOption<T>(value: unknown): T | null {
  if (typeof value === "object" && value !== null && "__option" in value) {
    const option = value as { __option: string; value?: T };
    if (option.__option === "None") return null;
    if (option.__option === "Some") return option.value as T;
    throw new Error("invalid option tag");
  }
  return value as T | null;
}

function codecSize(
  codec: { encode(value: any): ArrayLike<number> },
  value: unknown,
): number {
  const fixedSize = (codec as { fixedSize?: unknown }).fixedSize;
  return typeof fixedSize === "number" ? fixedSize : codec.encode(value).length;
}

function bytesEqual(left: ArrayLike<number>, right: ArrayLike<number>): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}

function decodeExact<T>(
  codec: { decode(data: Uint8Array): unknown; encode(value: any): ArrayLike<number> },
  data: Uint8Array,
): T {
  const value = codec.decode(data);
  if (!bytesEqual(codec.encode(value), data)) throw new Error("invalid or trailing bytes");
  return value as T;
}

function assertFinished(data: Uint8Array, offset: number): void {
  if (offset !== data.length) throw new Error("trailing bytes");
}

/* Constants */
export const PROGRAM_ADDRESS = address("33333333333333333333333333333333333333333333");
export const DEPOSIT_INSTRUCTION_DISCRIMINATOR = new Uint8Array([0]);
export const WITHDRAW_INSTRUCTION_DISCRIMINATOR = new Uint8Array([1]);

/* Interfaces */
export interface DepositInstructionArgs {
  amount: bigint;
}

export interface WithdrawInstructionArgs {
  amount: bigint;
}

export interface DepositInstructionInput {
  user: Address;
  amount: bigint;
}

export interface WithdrawInstructionInput {
  user: Address;
  amount: bigint;
}

export interface DepositInstructionAccountOverrides {
  user?: Address;
  vault?: Address;
  systemProgram?: Address;
}

export interface WithdrawInstructionAccountOverrides {
  user?: Address;
  vault?: Address;
  systemProgram?: Address;
}

/* Enums */
export const ProgramInstruction = {
  Deposit: "Deposit",
  Withdraw: "Withdraw",
} as const;

export type ProgramInstruction =
  (typeof ProgramInstruction)[keyof typeof ProgramInstruction];

export type DecodedInstruction =
  | { type: typeof ProgramInstruction.Deposit; args: DepositInstructionArgs }
  | { type: typeof ProgramInstruction.Withdraw; args: WithdrawInstructionArgs };

/* Client */
export class QuasarVaultClient {

  decodeInstruction(input: ArrayLike<number>): DecodedInstruction | null {
    const data = Uint8Array.from(input);
    if (matchDisc(data, DEPOSIT_INSTRUCTION_DISCRIMINATOR)) {
      const argsCodec = getStructCodec([
        ["amount", getU64Codec()],
      ]);
      return { type: ProgramInstruction.Deposit, args: decodeExact<DepositInstructionArgs>(argsCodec, data.slice(DEPOSIT_INSTRUCTION_DISCRIMINATOR.length)) };
    }
    if (matchDisc(data, WITHDRAW_INSTRUCTION_DISCRIMINATOR)) {
      const argsCodec = getStructCodec([
        ["amount", getU64Codec()],
      ]);
      return { type: ProgramInstruction.Withdraw, args: decodeExact<WithdrawInstructionArgs>(argsCodec, data.slice(WITHDRAW_INSTRUCTION_DISCRIMINATOR.length)) };
    }
    return null;
  }

  async createDepositInstruction(input: DepositInstructionInput): Promise<Instruction & { readonly vaultAddress: Address }> {
    return this.createDepositInstructionRaw(input, {});
  }

  async createDepositInstructionRaw(input: DepositInstructionInput, accountOverrides: DepositInstructionAccountOverrides): Promise<Instruction & { readonly vaultAddress: Address }> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["systemProgram"] = address("11111111111111111111111111111111");
    accountsMap["vault"] = await findVaultAddress((accountOverrides.user ?? input.user));
    const argsCodec = getStructCodec([
      ["amount", getU64Codec()],
    ]);
    const data = Uint8Array.from([0, ...argsCodec.encode({ amount: input.amount })]);
    return {
      programAddress: PROGRAM_ADDRESS,
      accounts: [
        { address: (accountOverrides.user ?? input.user), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.vault ?? accountsMap["vault"]), role: AccountRole.WRITABLE },
        { address: (accountOverrides.systemProgram ?? accountsMap["systemProgram"]), role: AccountRole.READONLY },
      ],
      data,
      vaultAddress: (accountOverrides.vault ?? accountsMap["vault"]),
    };
  }

  async createWithdrawInstruction(input: WithdrawInstructionInput): Promise<Instruction & { readonly vaultAddress: Address }> {
    return this.createWithdrawInstructionRaw(input, {});
  }

  async createWithdrawInstructionRaw(input: WithdrawInstructionInput, accountOverrides: WithdrawInstructionAccountOverrides): Promise<Instruction & { readonly vaultAddress: Address }> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["systemProgram"] = address("11111111111111111111111111111111");
    accountsMap["vault"] = await findVaultAddress((accountOverrides.user ?? input.user));
    const argsCodec = getStructCodec([
      ["amount", getU64Codec()],
    ]);
    const data = Uint8Array.from([1, ...argsCodec.encode({ amount: input.amount })]);
    return {
      programAddress: PROGRAM_ADDRESS,
      accounts: [
        { address: (accountOverrides.user ?? input.user), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.vault ?? accountsMap["vault"]), role: AccountRole.WRITABLE },
        { address: (accountOverrides.systemProgram ?? accountsMap["systemProgram"]), role: AccountRole.READONLY },
      ],
      data,
      vaultAddress: (accountOverrides.vault ?? accountsMap["vault"]),
    };
  }
}

/* Program Plugin */
export type QuasarVaultPluginRequirements = ClientWithPayer &
  ClientWithTransactionPlanning &
  ClientWithTransactionSending;

export function quasarVaultProgram() {
  const __client = new QuasarVaultClient();
  return <T extends QuasarVaultPluginRequirements>(client: T) => ({
    ...client,
    quasarVault: {
      instructions: {
        deposit: (input: DepositInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createDepositInstruction(input)),
        withdraw: (input: WithdrawInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createWithdrawInstruction(input)),
      },
    },
  });
}

/* PDA Helpers */
export async function findVaultAddress(user: Address): Promise<Address> {
  return (await getProgramDerivedAddress({
    programAddress: PROGRAM_ADDRESS,
    seeds: [
        new Uint8Array([118, 97, 117, 108, 116]),
      getAddressCodec().encode(user),
    ],
  }))[0];
}

/* Errors */
export const PROGRAM_ERROR_CODES = {
  AccountAlreadyInitialized: 3001,
  AccountNotInitialized: 3000,
  AccountNotMutable: 3010,
  AccountNotRentExempt: 3008,
  AccountNotSigner: 3011,
  AccountOwnedByWrongProgram: 3009,
  AddressMismatch: 3012,
  CompactWriterFieldNotSet: 3014,
  ConstraintViolation: 3004,
  DynamicFieldTooLong: 3013,
  HasOneMismatch: 3005,
  InsufficientSpace: 3007,
  InvalidDiscriminator: 3006,
  InvalidPda: 3002,
  InvalidReturnData: 3019,
  InvalidSeeds: 3003,
  MissingReturnData: 3017,
  RemainingAccountDuplicate: 3016,
  RemainingAccountsOverflow: 3015,
  ReturnDataFromWrongProgram: 3018,
} as const;

export const PROGRAM_ERRORS: Record<number, { name: string; msg?: string }> = {
  3001: { name: "AccountAlreadyInitialized", msg: "Account discriminator is already set (double-init attempt)." },
  3000: { name: "AccountNotInitialized", msg: "Account data is all zeros or has no discriminator." },
  3010: { name: "AccountNotMutable", msg: "Account was not passed as writable." },
  3008: { name: "AccountNotRentExempt", msg: "Account balance is below the rent-exemption minimum." },
  3011: { name: "AccountNotSigner", msg: "Account was not passed as a signer." },
  3009: { name: "AccountOwnedByWrongProgram", msg: "Account owner does not match the expected program." },
  3012: { name: "AddressMismatch", msg: "Account address does not match the expected value." },
  3014: { name: "CompactWriterFieldNotSet", msg: "A compact writer commit was attempted before setting every field." },
  3004: { name: "ConstraintViolation", msg: "A `#[account(constraint = ...)]` expression evaluated to false." },
  3013: { name: "DynamicFieldTooLong", msg: "A dynamic-length field exceeds its maximum byte length." },
  3005: { name: "HasOneMismatch", msg: "`#[account(has_one = ...)]` field does not match." },
  3007: { name: "InsufficientSpace", msg: "Account data is too small for the declared layout." },
  3006: { name: "InvalidDiscriminator", msg: "Account discriminator does not match the expected value." },
  3002: { name: "InvalidPda", msg: "PDA derivation does not match the expected address." },
  3019: { name: "InvalidReturnData", msg: "Return data bytes do not match the expected fixed-size layout." },
  3003: { name: "InvalidSeeds", msg: "Seeds provided for PDA verification are invalid." },
  3017: { name: "MissingReturnData", msg: "The callee completed successfully but did not set return data." },
  3016: { name: "RemainingAccountDuplicate", msg: "A duplicate remaining-account entry could not be resolved." },
  3015: { name: "RemainingAccountsOverflow", msg: "More remaining accounts than can fit in the buffer." },
  3018: { name: "ReturnDataFromWrongProgram", msg: "Return data was set by a different program than the one invoked." },
};
