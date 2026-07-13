import { Address, TransactionInstruction } from "@solana/web3.js";
import { fixCodecSize, getBytesCodec, getStructCodec, getU64Codec, getU8Codec, transformCodec } from "@solana/codecs";

function getWeb3jsAddressCodec() {
  return transformCodec(
    fixCodecSize(getBytesCodec(), 32),
    (value: Address) => value.toBytes(),
    bytes => new Address(bytes),
  );
}

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

/* Codecs */
const EscrowStructCodec = getStructCodec([
  ["maker", getWeb3jsAddressCodec()],
  ["mint_a", getWeb3jsAddressCodec()],
  ["mint_b", getWeb3jsAddressCodec()],
  ["maker_ta_b", getWeb3jsAddressCodec()],
  ["receive", getU64Codec()],
  ["bump", getU8Codec()],
]);
export const EscrowCodec = {
  ...EscrowStructCodec,
  decode(data: Parameters<typeof EscrowStructCodec.decode>[0], offset = 0): Escrow { return decodeExact<Escrow>(EscrowStructCodec, Uint8Array.from(data).slice(offset)); },
};

const MakeEventStructCodec = getStructCodec([
  ["escrow", getWeb3jsAddressCodec()],
  ["maker", getWeb3jsAddressCodec()],
  ["mint_a", getWeb3jsAddressCodec()],
  ["mint_b", getWeb3jsAddressCodec()],
  ["deposit", getU64Codec()],
  ["receive", getU64Codec()],
]);
export const MakeEventCodec = {
  ...MakeEventStructCodec,
  decode(data: Parameters<typeof MakeEventStructCodec.decode>[0], offset = 0): MakeEvent { return decodeExact<MakeEvent>(MakeEventStructCodec, Uint8Array.from(data).slice(offset)); },
};

const RefundEventStructCodec = getStructCodec([
  ["escrow", getWeb3jsAddressCodec()],
]);
export const RefundEventCodec = {
  ...RefundEventStructCodec,
  decode(data: Parameters<typeof RefundEventStructCodec.decode>[0], offset = 0): RefundEvent { return decodeExact<RefundEvent>(RefundEventStructCodec, Uint8Array.from(data).slice(offset)); },
};

const TakeEventStructCodec = getStructCodec([
  ["escrow", getWeb3jsAddressCodec()],
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
  static readonly programId = new Address("22222222222222222222222222222222222222222222");

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

  async createMakeInstruction(input: MakeInstructionInput): Promise<TransactionInstruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["rent"] = new Address("SysvarRent111111111111111111111111111111111");
    accountsMap["tokenProgram"] = new Address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    accountsMap["systemProgram"] = new Address("11111111111111111111111111111111");
    accountsMap["escrow"] = await findEscrowAddress(input.maker);
    const argsCodec = getStructCodec([
      ["deposit", getU64Codec()],
      ["receive", getU64Codec()],
    ]);
    const data = Uint8Array.from([0, ...argsCodec.encode({ deposit: input.deposit, receive: input.receive })]);
    return new TransactionInstruction({
      programId: QuasarEscrowClient.programId,
      keys: [
        { pubkey: input.maker, isSigner: true, isWritable: true },
        { pubkey: accountsMap["escrow"], isSigner: false, isWritable: true },
        { pubkey: input.mintA, isSigner: false, isWritable: false },
        { pubkey: input.mintB, isSigner: false, isWritable: false },
        { pubkey: input.makerTaA, isSigner: false, isWritable: true },
        { pubkey: input.makerTaB, isSigner: true, isWritable: true },
        { pubkey: input.vaultTaA, isSigner: true, isWritable: true },
        { pubkey: accountsMap["rent"], isSigner: false, isWritable: false },
        { pubkey: accountsMap["tokenProgram"], isSigner: false, isWritable: false },
        { pubkey: accountsMap["systemProgram"], isSigner: false, isWritable: false },
      ],
      data,
    });
  }

  async createTakeInstruction(input: TakeInstructionInput): Promise<TransactionInstruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["rent"] = new Address("SysvarRent111111111111111111111111111111111");
    accountsMap["tokenProgram"] = new Address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    accountsMap["systemProgram"] = new Address("11111111111111111111111111111111");
    accountsMap["escrow"] = await findEscrowAddress(input.maker);
    const data = Uint8Array.from([1]);
    return new TransactionInstruction({
      programId: QuasarEscrowClient.programId,
      keys: [
        { pubkey: input.taker, isSigner: true, isWritable: true },
        { pubkey: accountsMap["escrow"], isSigner: false, isWritable: true },
        { pubkey: input.maker, isSigner: false, isWritable: true },
        { pubkey: input.mintA, isSigner: false, isWritable: false },
        { pubkey: input.mintB, isSigner: false, isWritable: false },
        { pubkey: input.takerTaA, isSigner: true, isWritable: true },
        { pubkey: input.takerTaB, isSigner: false, isWritable: true },
        { pubkey: input.makerTaB, isSigner: true, isWritable: true },
        { pubkey: input.vaultTaA, isSigner: false, isWritable: true },
        { pubkey: accountsMap["rent"], isSigner: false, isWritable: false },
        { pubkey: accountsMap["tokenProgram"], isSigner: false, isWritable: false },
        { pubkey: accountsMap["systemProgram"], isSigner: false, isWritable: false },
      ],
      data,
    });
  }

  async createRefundInstruction(input: RefundInstructionInput): Promise<TransactionInstruction> {
    const accountsMap: Record<string, Address> = {};
    accountsMap["rent"] = new Address("SysvarRent111111111111111111111111111111111");
    accountsMap["tokenProgram"] = new Address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    accountsMap["systemProgram"] = new Address("11111111111111111111111111111111");
    accountsMap["escrow"] = await findEscrowAddress(input.maker);
    const data = Uint8Array.from([2]);
    return new TransactionInstruction({
      programId: QuasarEscrowClient.programId,
      keys: [
        { pubkey: input.maker, isSigner: true, isWritable: true },
        { pubkey: accountsMap["escrow"], isSigner: false, isWritable: true },
        { pubkey: input.mintA, isSigner: false, isWritable: false },
        { pubkey: input.makerTaA, isSigner: true, isWritable: true },
        { pubkey: input.vaultTaA, isSigner: false, isWritable: true },
        { pubkey: accountsMap["rent"], isSigner: false, isWritable: false },
        { pubkey: accountsMap["tokenProgram"], isSigner: false, isWritable: false },
        { pubkey: accountsMap["systemProgram"], isSigner: false, isWritable: false },
      ],
      data,
    });
  }
}

/* PDA Helpers */
export async function findEscrowAddress(maker: Address): Promise<Address> {
  return (await Address.findProgramAddress(
    [
        new Uint8Array([101, 115, 99, 114, 111, 119]),
      maker.toBytes(),
    ],
    QuasarEscrowClient.programId,
  ))[0];
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
