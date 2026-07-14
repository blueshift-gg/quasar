import { type Address, address, AccountRole, type Instruction, getProgramDerivedAddress, getAddressCodec, type ClientWithPayer, type ClientWithTransactionPlanning, type ClientWithTransactionSending } from "@solana/kit";
import { addSelfPlanAndSendFunctions } from "@solana/kit/program-client-core";
import { addCodecSizePrefix, getArrayCodec, getStructCodec, getU16Codec, getU64Codec, getU8Codec, getUtf8Codec } from "@solana/codecs";

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
export const PROGRAM_ADDRESS = address("44444444444444444444444444444444444444444444");
export const MULTISIG_CONFIG_DISCRIMINATOR = new Uint8Array([1]);
export const CREATE_INSTRUCTION_DISCRIMINATOR = new Uint8Array([0]);
export const DEPOSIT_INSTRUCTION_DISCRIMINATOR = new Uint8Array([1]);
export const SET_LABEL_INSTRUCTION_DISCRIMINATOR = new Uint8Array([2]);
export const EXECUTE_TRANSFER_INSTRUCTION_DISCRIMINATOR = new Uint8Array([3]);

/* Interfaces */
export interface MultisigConfig {
  creator: Address;
  threshold: number;
  bump: number;
  label: string;
  signers: Array<Address>;
}

export interface CreateInstructionArgs {
  threshold: number;
}

export interface DepositInstructionArgs {
  amount: bigint;
}

export interface SetLabelInstructionArgs {
  label: string;
}

export interface ExecuteTransferInstructionArgs {
  amount: bigint;
}

export interface CreateInstructionInput {
  creator: Address;
  threshold: number;
  remainingAccounts?: Array<{ address: Address; role: AccountRole }>;
}

export interface DepositInstructionInput {
  depositor: Address;
  config: Address;
  amount: bigint;
}

export interface SetLabelInstructionInput {
  creator: Address;
  label: string;
}

export interface ExecuteTransferInstructionInput {
  creator: Address;
  recipient: Address;
  amount: bigint;
  remainingAccounts?: Array<{ address: Address; role: AccountRole }>;
}

/* Codecs */
export const MultisigConfigCodec = {
  encode(value: MultisigConfig): Uint8Array {
    const fixedCodec = getStructCodec([
      ["creator", getAddressCodec()],
      ["threshold", getU8Codec()],
      ["bump", getU8Codec()],
    ]);
    const fixedBytes = fixedCodec.encode({ creator: value.creator, threshold: value.threshold, bump: value.bump });
    const labelBytes = new TextEncoder().encode(value.label);
    const labelPrefix = getU8Codec().encode(labelBytes.length);
    const signersPrefix = getU16Codec().encode(value.signers.length);
    const signersBytes = getArrayCodec(getAddressCodec(), { size: value.signers.length }).encode(value.signers);
    return Uint8Array.from([...fixedBytes, ...labelPrefix, ...signersPrefix, ...labelBytes, ...signersBytes]);
  },
  decode(data: Uint8Array): MultisigConfig {
    let offset = 0;
    const fixedCodec = getStructCodec([
      ["creator", getAddressCodec()],
      ["threshold", getU8Codec()],
      ["bump", getU8Codec()],
    ]);
    const fixedResult = fixedCodec.decode(data.slice(offset));
    const fixedSize = codecSize(fixedCodec, fixedResult);
    if (!bytesEqual(fixedCodec.encode(fixedResult), checkedTake(data, offset, fixedSize))) throw new Error("invalid fixed field encoding");
    offset += fixedSize;
    const labelLen = getU8Codec().decode(checkedTake(data, offset, 1));
    offset += 1;
    const signersLen = getU16Codec().decode(checkedTake(data, offset, 2));
    offset += 2;
    const labelSize = checkedLength(labelLen);
    const label = decodeUtf8(checkedTake(data, offset, labelSize));
    offset += labelSize;
    const signersCount = checkedElementCount(signersLen, data.length - offset);
    const signersCodec = getArrayCodec(getAddressCodec(), { size: signersCount });
    const signers = signersCodec.decode(data.slice(offset));
    offset += signersCodec.encode(signers).length;
    const result = { creator: fixedResult.creator, threshold: fixedResult.threshold, bump: fixedResult.bump, label, signers };
    assertFinished(data, offset);
    if (!bytesEqual(this.encode(result), data)) throw new Error("invalid field encoding");
    return result;
  },
};

/* Enums */
export const ProgramInstruction = {
  Create: "Create",
  Deposit: "Deposit",
  SetLabel: "SetLabel",
  ExecuteTransfer: "ExecuteTransfer",
} as const;

export type ProgramInstruction =
  (typeof ProgramInstruction)[keyof typeof ProgramInstruction];

export type DecodedInstruction =
  | { type: typeof ProgramInstruction.Create; args: CreateInstructionArgs }
  | { type: typeof ProgramInstruction.Deposit; args: DepositInstructionArgs }
  | { type: typeof ProgramInstruction.SetLabel; args: SetLabelInstructionArgs }
  | { type: typeof ProgramInstruction.ExecuteTransfer; args: ExecuteTransferInstructionArgs };

/* Client */
export class QuasarMultisigClient {

  decodeMultisigConfig(data: Uint8Array): MultisigConfig {
    if (!matchDisc(data, MULTISIG_CONFIG_DISCRIMINATOR)) throw new Error("Invalid MultisigConfig discriminator");
    return decodeExact<MultisigConfig>(MultisigConfigCodec, data.slice(MULTISIG_CONFIG_DISCRIMINATOR.length));
  }

  decodeInstruction(data: Uint8Array): DecodedInstruction | null {
    if (matchDisc(data, CREATE_INSTRUCTION_DISCRIMINATOR)) {
      const argsCodec = getStructCodec([
        ["threshold", getU8Codec()],
      ]);
      return { type: ProgramInstruction.Create, args: decodeExact<CreateInstructionArgs>(argsCodec, data.slice(CREATE_INSTRUCTION_DISCRIMINATOR.length)) };
    }
    if (matchDisc(data, DEPOSIT_INSTRUCTION_DISCRIMINATOR)) {
      const argsCodec = getStructCodec([
        ["amount", getU64Codec()],
      ]);
      return { type: ProgramInstruction.Deposit, args: decodeExact<DepositInstructionArgs>(argsCodec, data.slice(DEPOSIT_INSTRUCTION_DISCRIMINATOR.length)) };
    }
    if (matchDisc(data, SET_LABEL_INSTRUCTION_DISCRIMINATOR)) {
      let offset = SET_LABEL_INSTRUCTION_DISCRIMINATOR.length;
      const labelLen = getU8Codec().decode(checkedTake(data, offset, 1));
      offset += 1;
      const labelSize = checkedLength(labelLen);
      const label = decodeUtf8(checkedTake(data, offset, labelSize));
      offset += labelSize;
      assertFinished(data, offset);
      return { type: ProgramInstruction.SetLabel, args: { label } };
    }
    if (matchDisc(data, EXECUTE_TRANSFER_INSTRUCTION_DISCRIMINATOR)) {
      const argsCodec = getStructCodec([
        ["amount", getU64Codec()],
      ]);
      return { type: ProgramInstruction.ExecuteTransfer, args: decodeExact<ExecuteTransferInstructionArgs>(argsCodec, data.slice(EXECUTE_TRANSFER_INSTRUCTION_DISCRIMINATOR.length)) };
    }
    return null;
  }

  async createCreateInstruction(input: CreateInstructionInput): Promise<Instruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["rent"] = address("SysvarRent111111111111111111111111111111111");
    accountsMap["systemProgram"] = address("11111111111111111111111111111111");
    accountsMap["config"] = await findConfigAddress(input.creator);
    const argsCodec = getStructCodec([
      ["threshold", getU8Codec()],
    ]);
    const data = Uint8Array.from([0, ...argsCodec.encode({ threshold: input.threshold })]);
    return {
      programAddress: PROGRAM_ADDRESS,
      accounts: [
        { address: input.creator, role: AccountRole.WRITABLE_SIGNER },
        { address: accountsMap["config"], role: AccountRole.WRITABLE },
        { address: accountsMap["rent"], role: AccountRole.READONLY },
        { address: accountsMap["systemProgram"], role: AccountRole.READONLY },
        ...(input.remainingAccounts ?? []),
      ],
      data,
    };
  }

  async createDepositInstruction(input: DepositInstructionInput): Promise<Instruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["systemProgram"] = address("11111111111111111111111111111111");
    accountsMap["vault"] = await findVaultAddress(input.config);
    const argsCodec = getStructCodec([
      ["amount", getU64Codec()],
    ]);
    const data = Uint8Array.from([1, ...argsCodec.encode({ amount: input.amount })]);
    return {
      programAddress: PROGRAM_ADDRESS,
      accounts: [
        { address: input.depositor, role: AccountRole.WRITABLE_SIGNER },
        { address: input.config, role: AccountRole.READONLY },
        { address: accountsMap["vault"], role: AccountRole.WRITABLE },
        { address: accountsMap["systemProgram"], role: AccountRole.READONLY },
      ],
      data,
    };
  }

  async createSetLabelInstruction(input: SetLabelInstructionInput): Promise<Instruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["systemProgram"] = address("11111111111111111111111111111111");
    accountsMap["config"] = await findConfigAddress(input.creator);
    const disc = new Uint8Array([2]);
    const fixedBytes = new Uint8Array(0);
    const labelBytes = new TextEncoder().encode(input.label);
    const labelPrefix = getU8Codec().encode(labelBytes.length);
    const data = Uint8Array.from([...disc, ...fixedBytes, ...labelPrefix, ...labelBytes]);
    return {
      programAddress: PROGRAM_ADDRESS,
      accounts: [
        { address: input.creator, role: AccountRole.WRITABLE_SIGNER },
        { address: accountsMap["config"], role: AccountRole.WRITABLE },
        { address: accountsMap["systemProgram"], role: AccountRole.READONLY },
      ],
      data,
    };
  }

  async createExecuteTransferInstruction(input: ExecuteTransferInstructionInput): Promise<Instruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["systemProgram"] = address("11111111111111111111111111111111");
    accountsMap["config"] = await findConfigAddress(input.creator);
    accountsMap["vault"] = await findVaultAddress(accountsMap["config"]);
    const argsCodec = getStructCodec([
      ["amount", getU64Codec()],
    ]);
    const data = Uint8Array.from([3, ...argsCodec.encode({ amount: input.amount })]);
    return {
      programAddress: PROGRAM_ADDRESS,
      accounts: [
        { address: accountsMap["config"], role: AccountRole.READONLY },
        { address: input.creator, role: AccountRole.READONLY },
        { address: accountsMap["vault"], role: AccountRole.WRITABLE },
        { address: input.recipient, role: AccountRole.WRITABLE },
        { address: accountsMap["systemProgram"], role: AccountRole.READONLY },
        ...(input.remainingAccounts ?? []),
      ],
      data,
    };
  }
}

/* Program Plugin */
export type QuasarMultisigPluginRequirements = ClientWithPayer &
  ClientWithTransactionPlanning &
  ClientWithTransactionSending;

export function quasarMultisigProgram() {
  const __client = new QuasarMultisigClient();
  return <T extends QuasarMultisigPluginRequirements>(client: T) => ({
    ...client,
    quasarMultisig: {
      instructions: {
        create: (input: CreateInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createCreateInstruction(input)),
        deposit: (input: DepositInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createDepositInstruction(input)),
        setLabel: (input: SetLabelInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createSetLabelInstruction(input)),
        executeTransfer: (input: ExecuteTransferInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createExecuteTransferInstruction(input)),
      },
    },
  });
}

/* PDA Helpers */
export async function findConfigAddress(creator: Address): Promise<Address> {
  return (await getProgramDerivedAddress({
    programAddress: PROGRAM_ADDRESS,
    seeds: [
        new Uint8Array([109, 117, 108, 116, 105, 115, 105, 103]),
      getAddressCodec().encode(creator),
    ],
  }))[0];
}

export async function findVaultAddress(config: Address): Promise<Address> {
  return (await getProgramDerivedAddress({
    programAddress: PROGRAM_ADDRESS,
    seeds: [
        new Uint8Array([118, 97, 117, 108, 116]),
      getAddressCodec().encode(config),
    ],
  }))[0];
}

/* Errors */
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
