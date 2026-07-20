import { type Address, address, AccountRole, type Instruction, getProgramDerivedAddress, getAddressCodec, type ClientWithRpc, type GetAccountInfoApi, type GetMultipleAccountsApi, type ClientWithPayer, type ClientWithTransactionPlanning, type ClientWithTransactionSending } from "@solana/kit";
import { addSelfFetchFunctions, addSelfPlanAndSendFunctions } from "@solana/kit/program-client-core";
import { getStructCodec, getU64Codec, getU8Codec } from "@solana/codecs";

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
export const PROGRAM_ADDRESS = address("22222222222222222222222222222222222222222222");
export const ESCROW_DISCRIMINATOR = new Uint8Array([1]);
export const MAKE_EVENT_DISCRIMINATOR = new Uint8Array([1]);
export const REFUND_EVENT_DISCRIMINATOR = new Uint8Array([3]);
export const TAKE_EVENT_DISCRIMINATOR = new Uint8Array([2]);
export const MAKE_INSTRUCTION_DISCRIMINATOR = new Uint8Array([0]);
export const TAKE_INSTRUCTION_DISCRIMINATOR = new Uint8Array([1]);
export const REFUND_INSTRUCTION_DISCRIMINATOR = new Uint8Array([2]);

/* Interfaces */
export interface Escrow {
  maker: Address;
  mint_a: Address;
  mint_b: Address;
  maker_ta_b: Address;
  receive: bigint;
  bump: number;
}

export interface MakeEvent {
  escrow: Address;
  maker: Address;
  mint_a: Address;
  mint_b: Address;
  deposit: bigint;
  receive: bigint;
}

export interface RefundEvent {
  escrow: Address;
}

export interface TakeEvent {
  escrow: Address;
}

export interface MakeInstructionArgs {
  deposit: bigint;
  receive: bigint;
}

export interface MakeInstructionInput {
  maker: Address;
  mintA: Address;
  mintB: Address;
  makerTaA: Address;
  makerTaB: Address;
  vaultTaA: Address;
  deposit: bigint;
  receive: bigint;
}

export interface TakeInstructionInput {
  taker: Address;
  maker: Address;
  mintA: Address;
  mintB: Address;
  takerTaA: Address;
  takerTaB: Address;
  makerTaB: Address;
  vaultTaA: Address;
}

export interface RefundInstructionInput {
  maker: Address;
  mintA: Address;
  makerTaA: Address;
  vaultTaA: Address;
}

export interface MakeInstructionAccountOverrides {
  maker?: Address;
  escrow?: Address;
  mintA?: Address;
  mintB?: Address;
  makerTaA?: Address;
  makerTaB?: Address;
  vaultTaA?: Address;
  rent?: Address;
  tokenProgram?: Address;
  systemProgram?: Address;
}

export interface TakeInstructionAccountOverrides {
  taker?: Address;
  escrow?: Address;
  maker?: Address;
  mintA?: Address;
  mintB?: Address;
  takerTaA?: Address;
  takerTaB?: Address;
  makerTaB?: Address;
  vaultTaA?: Address;
  rent?: Address;
  tokenProgram?: Address;
  systemProgram?: Address;
}

export interface RefundInstructionAccountOverrides {
  maker?: Address;
  escrow?: Address;
  mintA?: Address;
  makerTaA?: Address;
  vaultTaA?: Address;
  rent?: Address;
  tokenProgram?: Address;
  systemProgram?: Address;
}

/* Codecs */
const EscrowStructCodec = getStructCodec([
  ["maker", getAddressCodec()],
  ["mint_a", getAddressCodec()],
  ["mint_b", getAddressCodec()],
  ["maker_ta_b", getAddressCodec()],
  ["receive", getU64Codec()],
  ["bump", getU8Codec()],
]);
export const EscrowCodec = {
  ...EscrowStructCodec,
  decode(data: Parameters<typeof EscrowStructCodec.decode>[0], offset = 0): Escrow { return decodeExact<Escrow>(EscrowStructCodec, Uint8Array.from(data).slice(offset)); },
};

const MakeEventStructCodec = getStructCodec([
  ["escrow", getAddressCodec()],
  ["maker", getAddressCodec()],
  ["mint_a", getAddressCodec()],
  ["mint_b", getAddressCodec()],
  ["deposit", getU64Codec()],
  ["receive", getU64Codec()],
]);
export const MakeEventCodec = {
  ...MakeEventStructCodec,
  decode(data: Parameters<typeof MakeEventStructCodec.decode>[0], offset = 0): MakeEvent { return decodeExact<MakeEvent>(MakeEventStructCodec, Uint8Array.from(data).slice(offset)); },
};

const RefundEventStructCodec = getStructCodec([
  ["escrow", getAddressCodec()],
]);
export const RefundEventCodec = {
  ...RefundEventStructCodec,
  decode(data: Parameters<typeof RefundEventStructCodec.decode>[0], offset = 0): RefundEvent { return decodeExact<RefundEvent>(RefundEventStructCodec, Uint8Array.from(data).slice(offset)); },
};

const TakeEventStructCodec = getStructCodec([
  ["escrow", getAddressCodec()],
]);
export const TakeEventCodec = {
  ...TakeEventStructCodec,
  decode(data: Parameters<typeof TakeEventStructCodec.decode>[0], offset = 0): TakeEvent { return decodeExact<TakeEvent>(TakeEventStructCodec, Uint8Array.from(data).slice(offset)); },
};

/* Enums */
export const ProgramEvent = {
  MakeEvent: "MakeEvent",
  RefundEvent: "RefundEvent",
  TakeEvent: "TakeEvent",
} as const;

export type ProgramEvent =
  (typeof ProgramEvent)[keyof typeof ProgramEvent];

export type DecodedEvent =
  | { type: typeof ProgramEvent.MakeEvent; data: MakeEvent }
  | { type: typeof ProgramEvent.RefundEvent; data: RefundEvent }
  | { type: typeof ProgramEvent.TakeEvent; data: TakeEvent };

export const ProgramInstruction = {
  Make: "Make",
  Take: "Take",
  Refund: "Refund",
} as const;

export type ProgramInstruction =
  (typeof ProgramInstruction)[keyof typeof ProgramInstruction];

export type DecodedInstruction =
  | { type: typeof ProgramInstruction.Make; args: MakeInstructionArgs }
  | { type: typeof ProgramInstruction.Take }
  | { type: typeof ProgramInstruction.Refund };

/* Client */
export class QuasarEscrowClient {

  decodeEscrow(data: Uint8Array): Escrow {
    if (!matchDisc(data, ESCROW_DISCRIMINATOR)) throw new Error("Invalid Escrow discriminator");
    return decodeExact<Escrow>(EscrowCodec, data.slice(ESCROW_DISCRIMINATOR.length));
  }

  decodeEvent(data: Uint8Array): DecodedEvent | null {
    if (matchDisc(data, MAKE_EVENT_DISCRIMINATOR))
      return { type: ProgramEvent.MakeEvent, data: decodeExact<MakeEvent>(MakeEventCodec, data.slice(MAKE_EVENT_DISCRIMINATOR.length)) };
    if (matchDisc(data, REFUND_EVENT_DISCRIMINATOR))
      return { type: ProgramEvent.RefundEvent, data: decodeExact<RefundEvent>(RefundEventCodec, data.slice(REFUND_EVENT_DISCRIMINATOR.length)) };
    if (matchDisc(data, TAKE_EVENT_DISCRIMINATOR))
      return { type: ProgramEvent.TakeEvent, data: decodeExact<TakeEvent>(TakeEventCodec, data.slice(TAKE_EVENT_DISCRIMINATOR.length)) };
    return null;
  }

  decodeInstruction(data: Uint8Array): DecodedInstruction | null {
    if (matchDisc(data, MAKE_INSTRUCTION_DISCRIMINATOR)) {
      const argsCodec = getStructCodec([
        ["deposit", getU64Codec()],
        ["receive", getU64Codec()],
      ]);
      return { type: ProgramInstruction.Make, args: decodeExact<MakeInstructionArgs>(argsCodec, data.slice(MAKE_INSTRUCTION_DISCRIMINATOR.length)) };
    }
    if (matchDisc(data, TAKE_INSTRUCTION_DISCRIMINATOR)) {
      if (data.length !== TAKE_INSTRUCTION_DISCRIMINATOR.length) throw new Error("trailing bytes");
      return { type: ProgramInstruction.Take };
    }
    if (matchDisc(data, REFUND_INSTRUCTION_DISCRIMINATOR)) {
      if (data.length !== REFUND_INSTRUCTION_DISCRIMINATOR.length) throw new Error("trailing bytes");
      return { type: ProgramInstruction.Refund };
    }
    return null;
  }

  async createMakeInstruction(input: MakeInstructionInput): Promise<Instruction> {
    return this.createMakeInstructionUnchecked(input, {});
  }

  async createMakeInstructionUnchecked(input: MakeInstructionInput, accountOverrides: MakeInstructionAccountOverrides): Promise<Instruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["rent"] = address("SysvarRent111111111111111111111111111111111");
    accountsMap["tokenProgram"] = address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    accountsMap["systemProgram"] = address("11111111111111111111111111111111");
    accountsMap["escrow"] = await findEscrowAddress((accountOverrides.maker ?? input.maker));
    const argsCodec = getStructCodec([
      ["deposit", getU64Codec()],
      ["receive", getU64Codec()],
    ]);
    const data = Uint8Array.from([0, ...argsCodec.encode({ deposit: input.deposit, receive: input.receive })]);
    return {
      programAddress: PROGRAM_ADDRESS,
      accounts: [
        { address: (accountOverrides.maker ?? input.maker), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.escrow ?? accountsMap["escrow"]), role: AccountRole.WRITABLE },
        { address: (accountOverrides.mintA ?? input.mintA), role: AccountRole.READONLY },
        { address: (accountOverrides.mintB ?? input.mintB), role: AccountRole.READONLY },
        { address: (accountOverrides.makerTaA ?? input.makerTaA), role: AccountRole.WRITABLE },
        { address: (accountOverrides.makerTaB ?? input.makerTaB), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.vaultTaA ?? input.vaultTaA), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.rent ?? accountsMap["rent"]), role: AccountRole.READONLY },
        { address: (accountOverrides.tokenProgram ?? accountsMap["tokenProgram"]), role: AccountRole.READONLY },
        { address: (accountOverrides.systemProgram ?? accountsMap["systemProgram"]), role: AccountRole.READONLY },
      ],
      data,
    };
  }

  async createTakeInstruction(input: TakeInstructionInput): Promise<Instruction> {
    return this.createTakeInstructionUnchecked(input, {});
  }

  async createTakeInstructionUnchecked(input: TakeInstructionInput, accountOverrides: TakeInstructionAccountOverrides): Promise<Instruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["rent"] = address("SysvarRent111111111111111111111111111111111");
    accountsMap["tokenProgram"] = address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    accountsMap["systemProgram"] = address("11111111111111111111111111111111");
    accountsMap["escrow"] = await findEscrowAddress((accountOverrides.maker ?? input.maker));
    const data = Uint8Array.from([1]);
    return {
      programAddress: PROGRAM_ADDRESS,
      accounts: [
        { address: (accountOverrides.taker ?? input.taker), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.escrow ?? accountsMap["escrow"]), role: AccountRole.WRITABLE },
        { address: (accountOverrides.maker ?? input.maker), role: AccountRole.WRITABLE },
        { address: (accountOverrides.mintA ?? input.mintA), role: AccountRole.READONLY },
        { address: (accountOverrides.mintB ?? input.mintB), role: AccountRole.READONLY },
        { address: (accountOverrides.takerTaA ?? input.takerTaA), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.takerTaB ?? input.takerTaB), role: AccountRole.WRITABLE },
        { address: (accountOverrides.makerTaB ?? input.makerTaB), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.vaultTaA ?? input.vaultTaA), role: AccountRole.WRITABLE },
        { address: (accountOverrides.rent ?? accountsMap["rent"]), role: AccountRole.READONLY },
        { address: (accountOverrides.tokenProgram ?? accountsMap["tokenProgram"]), role: AccountRole.READONLY },
        { address: (accountOverrides.systemProgram ?? accountsMap["systemProgram"]), role: AccountRole.READONLY },
      ],
      data,
    };
  }

  async createRefundInstruction(input: RefundInstructionInput): Promise<Instruction> {
    return this.createRefundInstructionUnchecked(input, {});
  }

  async createRefundInstructionUnchecked(input: RefundInstructionInput, accountOverrides: RefundInstructionAccountOverrides): Promise<Instruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["rent"] = address("SysvarRent111111111111111111111111111111111");
    accountsMap["tokenProgram"] = address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    accountsMap["systemProgram"] = address("11111111111111111111111111111111");
    accountsMap["escrow"] = await findEscrowAddress((accountOverrides.maker ?? input.maker));
    const data = Uint8Array.from([2]);
    return {
      programAddress: PROGRAM_ADDRESS,
      accounts: [
        { address: (accountOverrides.maker ?? input.maker), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.escrow ?? accountsMap["escrow"]), role: AccountRole.WRITABLE },
        { address: (accountOverrides.mintA ?? input.mintA), role: AccountRole.READONLY },
        { address: (accountOverrides.makerTaA ?? input.makerTaA), role: AccountRole.WRITABLE_SIGNER },
        { address: (accountOverrides.vaultTaA ?? input.vaultTaA), role: AccountRole.WRITABLE },
        { address: (accountOverrides.rent ?? accountsMap["rent"]), role: AccountRole.READONLY },
        { address: (accountOverrides.tokenProgram ?? accountsMap["tokenProgram"]), role: AccountRole.READONLY },
        { address: (accountOverrides.systemProgram ?? accountsMap["systemProgram"]), role: AccountRole.READONLY },
      ],
      data,
    };
  }
}

/* Program Plugin */
export type QuasarEscrowPluginRequirements = ClientWithRpc<GetAccountInfoApi & GetMultipleAccountsApi> &
  ClientWithPayer &
  ClientWithTransactionPlanning &
  ClientWithTransactionSending;

export function quasarEscrowProgram() {
  const __client = new QuasarEscrowClient();
  return <T extends QuasarEscrowPluginRequirements>(client: T) => ({
    ...client,
    quasarEscrow: {
      accounts: {
        escrow: addSelfFetchFunctions(client, EscrowCodec),
      },
      instructions: {
        make: (input: MakeInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createMakeInstruction(input)),
        take: (input: TakeInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createTakeInstruction(input)),
        refund: (input: RefundInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createRefundInstruction(input)),
      },
    },
  });
}

/* PDA Helpers */
export async function findEscrowAddress(maker: Address): Promise<Address> {
  return (await getProgramDerivedAddress({
    programAddress: PROGRAM_ADDRESS,
    seeds: [
        new Uint8Array([101, 115, 99, 114, 111, 119]),
      getAddressCodec().encode(maker),
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
