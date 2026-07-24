import {
  ComputeBudget,
  FailedTransactionMetadata,
  LiteSVM,
  SimulatedTransactionInfo,
} from "litesvm";
import {
  AccountRole,
  address,
  appendTransactionMessageInstructions,
  compileTransaction,
  createTransactionMessage,
  getAddressDecoder,
  lamports,
  pipe,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  type Blockhash,
} from "@solana/kit";
import { SYSTEM_PROGRAM_ID } from "./spl.js";
import type { HarnessResult, HarnessRuntime } from "./test.js";
import type { ProgramError } from "./outcome.js";

// ---------------------------------------------------------------------------
// Backend-neutral wire types the runtime operates on. Each adapter converts its
// native address/account/instruction types to and from these.
// ---------------------------------------------------------------------------

/** An instruction reduced to base58 addresses and raw account roles. */
export interface RtInstruction {
  readonly programAddress: string;
  readonly accounts: readonly {
    readonly address: string;
    readonly signer: boolean;
    readonly writable: boolean;
  }[];
  readonly data: Uint8Array;
}

/** An account reduced to base58 addresses and raw bytes. */
export interface RtAccount {
  readonly address: string;
  readonly owner: string;
  readonly lamports: bigint;
  readonly data: Uint8Array;
  readonly executable: boolean;
}

/** Adapter glue converting native types to and from the neutral wire types. */
export interface LiteSvmConverters<Address, Account, Instruction> {
  addressString(value: Address): string;
  instructionToRt(instruction: Instruction): RtInstruction;
  accountToRt(account: Account): RtAccount;
  buildAccount(account: RtAccount): Account;
}

// ---------------------------------------------------------------------------
// Fee neutrality
// ---------------------------------------------------------------------------

// The default fee structure charges one signature's worth of lamports per
// required signature (verified against LiteSVM 1.3 / agave 4.1). Exact-lamport
// assertions must keep passing, so the fee payer is pre-funded by exactly the
// fee before every send; the runtime deducts it back and the net balance is
// unchanged. On failure LiteSVM commits nothing, so the pre-funding is invisible.
const LAMPORTS_PER_SIGNATURE = 5000n;

// A deterministic inert fee payer for transactions that name no signer. Never
// tracked, so its balance is irrelevant; it only needs to cover the fee. Mirrors
// the Rust backend's `b"quasar-test/fee-payer"` derivation.
const FEE_PAYER_ADDRESS = (() => {
  const bytes = new Uint8Array(32);
  bytes.set(new TextEncoder().encode("quasar-test/fee-payer"));
  return getAddressDecoder().decode(bytes);
})();
const FEE_PAYER_FUNDING = 1_000_000_000_000n;

// The upgradeable BPF loader is LiteSVM's default; loader v2 uses BPFLoader2.
const BPF_LOADER_2 = "BPFLoader2111111111111111111111111111111111";

// LiteSVM (kit v6) and the harness (kit v7) share byte-identical wire formats
// but nominally distinct branded types; cast at these boundaries only.
type SvmTransaction = Parameters<LiteSVM["sendTransaction"]>[0];
type SvmEncodedAccount = Parameters<LiteSVM["setAccount"]>[0];
type SvmAddress = Parameters<LiteSVM["getAccount"]>[0];

interface EncodedLike {
  address: string;
  programAddress: string;
  lamports: bigint | number;
  data: Uint8Array;
  executable: boolean;
}

/**
 * A `HarnessRuntime` backed by the official `litesvm` npm package. Instructions
 * and accounts enter as native types, are lowered to legacy transactions signed
 * with zero signatures (sigverify and blockhash checks disabled), executed, and
 * lifted back to native accounts and a `RawExecutionResult`.
 */
export class LiteSvmRuntime<Address, Account, Instruction>
  implements HarnessRuntime<Address, Account, Instruction>
{
  readonly #svm: LiteSVM;
  readonly #convert: LiteSvmConverters<Address, Account, Instruction>;

  constructor(convert: LiteSvmConverters<Address, Account, Instruction>) {
    this.#convert = convert;
    // Loading with default programs pulls in SPL Token, Token-2022 and the
    // Associated Token program, matching the previous backend's full config.
    this.#svm = new LiteSVM();
    this.#svm.withSigverify(false);
    this.#svm.withBlockhashCheck(false);
  }

  addProgram(programId: Address, elf: Uint8Array, loaderVersion?: number): void {
    const id = address(this.#convert.addressString(programId));
    if (loaderVersion === 2) {
      this.#svm.addProgramWithLoader(id, elf, address(BPF_LOADER_2));
    } else {
      this.#svm.addProgram(id, elf);
    }
  }

  setComputeBudget(maxUnits: bigint): void {
    const budget = new ComputeBudget();
    budget.computeUnitLimit = maxUnits;
    this.#svm.withComputeBudget(budget);
  }

  warpToTimestamp(timestamp: bigint): void {
    const clock = this.#svm.getClock();
    clock.unixTimestamp = timestamp;
    this.#svm.setClock(clock);
  }

  free(): void {
    // LiteSVM is a plain napi object reclaimed by the GC; nothing to free.
  }

  processInstructionChain(
    instructions: Instruction[],
    accounts: Account[],
  ): HarnessResult<Account> {
    return this.#execute(instructions, accounts, true);
  }

  simulateInstructionChain(
    instructions: Instruction[],
    accounts: Account[],
  ): HarnessResult<Account> {
    return this.#execute(instructions, accounts, false);
  }

  #execute(
    instructions: Instruction[],
    accounts: Account[],
    commit: boolean,
  ): HarnessResult<Account> {
    const rtInstructions = instructions.map(instruction =>
      this.#convert.instructionToRt(instruction),
    );
    const rtAccounts = accounts.map(account =>
      this.#convert.accountToRt(account),
    );

    // Collect signer/writable status and first-seen order across the chain.
    const signers = new Set<string>();
    const writable = new Set<string>();
    const order: string[] = [];
    const seen = new Set<string>();
    for (const instruction of rtInstructions) {
      for (const meta of instruction.accounts) {
        if (meta.signer) signers.add(meta.address);
        if (meta.writable) writable.add(meta.address);
        if (!seen.has(meta.address)) {
          seen.add(meta.address);
          order.push(meta.address);
        }
      }
    }

    // The fee payer is the first writable signer, else the first signer, else a
    // synthesized inert payer. It is placed first in the compiled message.
    let feePayer =
      order.find(a => signers.has(a) && writable.has(a)) ??
      order.find(a => signers.has(a));
    const synthesized = feePayer === undefined;
    if (feePayer === undefined) feePayer = FEE_PAYER_ADDRESS;

    const signatureCount = BigInt(signers.size + (signers.has(feePayer) ? 0 : 1));
    const fee = signatureCount * LAMPORTS_PER_SIGNATURE;

    // Seed every input account, pre-funding the fee payer by exactly the fee.
    const provided = new Set<string>();
    for (const account of rtAccounts) {
      provided.add(account.address);
      const extra = account.address === feePayer ? fee : 0n;
      this.#seed(account.address, account.owner, account.lamports + extra, account.data, account.executable);
    }
    if (!provided.has(feePayer)) {
      // A signer the harness did not install (or the synthesized payer): fund it
      // enough to cover the fee without perturbing any tracked account.
      const funding = synthesized ? FEE_PAYER_FUNDING : FEE_PAYER_FUNDING + fee;
      this.#seed(feePayer, SYSTEM_PROGRAM_ID, funding, new Uint8Array(0), false);
    }

    const feePayerAddress = address(feePayer);
    const kitInstructions = rtInstructions.map(instruction => ({
      programAddress: address(instruction.programAddress),
      accounts: instruction.accounts.map(meta => ({
        address: address(meta.address),
        role: roleOf(meta.signer, meta.writable),
      })),
      data: instruction.data,
    }));
    const blockhash = this.#svm.latestBlockhash() as unknown as Blockhash;
    const message = pipe(
      createTransactionMessage({ version: "legacy" }),
      m => setTransactionMessageFeePayer(feePayerAddress, m),
      m => appendTransactionMessageInstructions(kitInstructions, m),
      m =>
        setTransactionMessageLifetimeUsingBlockhash(
          { blockhash, lastValidBlockHeight: 0n },
          m,
        ),
    );
    const transaction = compileTransaction(message) as unknown as SvmTransaction;

    if (commit) {
      const result = this.#svm.sendTransaction(transaction);
      if (result instanceof FailedTransactionMetadata) {
        return this.#failure(result);
      }
      return {
        status: { ok: true },
        computeUnits: result.computeUnitsConsumed(),
        logs: result.logs(),
        returnData: readReturnData(result),
        accounts: rtAccounts.map(account => this.#committed(account)),
      };
    }

    const result = this.#svm.simulateTransaction(transaction);
    if (result instanceof FailedTransactionMetadata) {
      return this.#failure(result);
    }
    const info = result as SimulatedTransactionInfo;
    const meta = info.meta();
    const post = new Map<string, EncodedLike>();
    for (const encoded of info.postAccounts() as unknown as EncodedLike[]) {
      post.set(encoded.address, encoded);
    }
    return {
      status: { ok: true },
      computeUnits: meta.computeUnitsConsumed(),
      logs: meta.logs(),
      returnData: readReturnData(meta),
      // Simulation does not commit, so untouched inputs keep their pre-state.
      accounts: rtAccounts.map(account => {
        const encoded = post.get(account.address);
        return encoded === undefined
          ? this.#convert.buildAccount(account)
          : this.#convert.buildAccount(fromEncoded(encoded));
      }),
    };
  }

  #seed(
    address_: string,
    owner: string,
    lamps: bigint,
    data: Uint8Array,
    executable: boolean,
  ): void {
    this.#svm.setAccount({
      address: address(address_),
      programAddress: address(owner),
      lamports: lamports(lamps),
      data,
      executable,
      space: BigInt(data.length),
    } as unknown as SvmEncodedAccount);
  }

  #committed(input: RtAccount): Account {
    const account = this.#svm.getAccount(
      address(input.address) as unknown as SvmAddress,
    ) as unknown as (EncodedLike & { exists: boolean }) | { exists: false };
    if (account.exists) {
      return this.#convert.buildAccount(fromEncoded(account as EncodedLike));
    }
    // A closed or absent account reads back as an empty system account, matching
    // how `isClosed` and change detection expect a removed account to look.
    return this.#convert.buildAccount({
      address: input.address,
      owner: SYSTEM_PROGRAM_ID,
      lamports: 0n,
      data: new Uint8Array(0),
      executable: false,
    });
  }

  #failure(result: FailedTransactionMetadata): HarnessResult<Account> {
    const meta = result.meta();
    return {
      status: { ok: false, error: mapTransactionError(result.err()) },
      computeUnits: meta.computeUnitsConsumed(),
      logs: meta.logs(),
      returnData: readReturnData(meta),
      accounts: [],
    };
  }
}

function roleOf(signer: boolean, writable: boolean): AccountRole {
  if (writable) {
    return signer ? AccountRole.WRITABLE_SIGNER : AccountRole.WRITABLE;
  }
  return signer ? AccountRole.READONLY_SIGNER : AccountRole.READONLY;
}

function fromEncoded(encoded: EncodedLike): RtAccount {
  return {
    address: String(encoded.address),
    owner: String(encoded.programAddress),
    lamports: BigInt(encoded.lamports),
    data: encoded.data as Uint8Array,
    executable: encoded.executable,
  };
}

interface MetaLike {
  returnData(): { data(): Uint8Array } | null;
}

function readReturnData(meta: MetaLike): Uint8Array {
  try {
    const returnData = meta.returnData();
    const data = returnData?.data();
    return data ? (data as Uint8Array) : new Uint8Array(0);
  } catch {
    return new Uint8Array(0);
  }
}

// ---------------------------------------------------------------------------
// Error mapping — mirrors the Rust `From<InstructionError> for ProgramError`.
// LiteSVM surfaces fieldless InstructionError variants as raw numbers (the
// `const enum` has no runtime object) and Custom/BorshIo as small objects.
// ---------------------------------------------------------------------------

/** Fieldless `InstructionError` variants that map to a named `ProgramError`. */
const NAMED_INSTRUCTION_ERROR: Readonly<Record<number, ProgramError["type"]>> = {
  1: "InvalidArgument",
  2: "InvalidInstructionData",
  3: "InvalidAccountData",
  4: "AccountDataTooSmall",
  5: "InsufficientFunds",
  6: "IncorrectProgramId",
  7: "MissingRequiredSignature",
  8: "AccountAlreadyInitialized",
  9: "UninitializedAccount",
  19: "MissingAccount", // NotEnoughAccountKeys
  31: "MissingAccount",
  34: "InvalidSeeds",
  36: "ComputeBudgetExceeded", // ComputationalBudgetExceeded
  41: "Immutable",
  42: "IncorrectAuthority",
  43: "AccountNotRentExempt",
  44: "InvalidAccountOwner",
  45: "ArithmeticOverflow",
  52: "BorshIoError",
};

/** Names of every fieldless `InstructionError` variant, for `Runtime` messages. */
const INSTRUCTION_ERROR_NAME: Readonly<Record<number, string>> = {
  0: "GenericError",
  1: "InvalidArgument",
  2: "InvalidInstructionData",
  3: "InvalidAccountData",
  4: "AccountDataTooSmall",
  5: "InsufficientFunds",
  6: "IncorrectProgramId",
  7: "MissingRequiredSignature",
  8: "AccountAlreadyInitialized",
  9: "UninitializedAccount",
  10: "UnbalancedInstruction",
  11: "ModifiedProgramId",
  12: "ExternalAccountLamportSpend",
  13: "ExternalAccountDataModified",
  14: "ReadonlyLamportChange",
  15: "ReadonlyDataModified",
  16: "DuplicateAccountIndex",
  17: "ExecutableModified",
  18: "RentEpochModified",
  19: "NotEnoughAccountKeys",
  20: "AccountDataSizeChanged",
  21: "AccountNotExecutable",
  22: "AccountBorrowFailed",
  23: "AccountBorrowOutstanding",
  24: "DuplicateAccountOutOfSync",
  25: "InvalidError",
  26: "ExecutableDataModified",
  27: "ExecutableLamportChange",
  28: "ExecutableAccountNotRentExempt",
  29: "UnsupportedProgramId",
  30: "CallDepth",
  31: "MissingAccount",
  32: "ReentrancyNotAllowed",
  33: "MaxSeedLengthExceeded",
  34: "InvalidSeeds",
  35: "InvalidRealloc",
  36: "ComputationalBudgetExceeded",
  37: "PrivilegeEscalation",
  38: "ProgramEnvironmentSetupFailure",
  39: "ProgramFailedToComplete",
  40: "ProgramFailedToCompile",
  41: "Immutable",
  42: "IncorrectAuthority",
  43: "AccountNotRentExempt",
  44: "InvalidAccountOwner",
  45: "ArithmeticOverflow",
  46: "UnsupportedSysvar",
  47: "IllegalOwner",
  48: "MaxAccountsDataAllocationsExceeded",
  49: "MaxAccountsExceeded",
  50: "MaxInstructionTraceLengthExceeded",
  51: "BuiltinProgramsMustConsumeComputeUnits",
  52: "BorshIoError",
};

const TRANSACTION_ERROR_NAME: Readonly<Record<number, string>> = {
  0: "AccountInUse",
  1: "AccountLoadedTwice",
  2: "AccountNotFound",
  3: "ProgramAccountNotFound",
  4: "InsufficientFundsForFee",
  5: "InvalidAccountForFee",
  6: "AlreadyProcessed",
  7: "BlockhashNotFound",
  8: "CallChainTooDeep",
  9: "MissingSignatureForFee",
  10: "InvalidAccountIndex",
  11: "SignatureFailure",
  12: "InvalidProgramForExecution",
  13: "SanitizeFailure",
  14: "ClusterMaintenance",
  15: "AccountBorrowOutstanding",
  16: "WouldExceedMaxBlockCostLimit",
  17: "UnsupportedVersion",
  18: "InvalidWritableAccount",
  19: "WouldExceedMaxAccountCostLimit",
  20: "WouldExceedAccountDataBlockLimit",
  21: "TooManyAccountLocks",
  22: "AddressLookupTableNotFound",
  23: "InvalidAddressLookupTableOwner",
  24: "InvalidAddressLookupTableData",
  25: "InvalidAddressLookupTableIndex",
  26: "InvalidRentPayingAccount",
  27: "WouldExceedMaxVoteCostLimit",
  28: "WouldExceedAccountDataTotalLimit",
  29: "MaxLoadedAccountsDataSizeExceeded",
  30: "ResanitizationNeeded",
  31: "InvalidLoadedAccountsDataSizeLimit",
  32: "UnbalancedTransaction",
  33: "ProgramCacheHitMaxLimit",
  34: "CommitCancelled",
};

/** A `FailedTransactionMetadata.err()` value: fieldless enum, or a small object. */
type TransactionErrorValue =
  | number
  | {
      readonly index?: number;
      readonly accountIndex?: number;
      err?(): InstructionErrorValue;
    };

type InstructionErrorValue =
  | number
  | { readonly code?: number; readonly msg?: string };

function mapTransactionError(error: TransactionErrorValue): ProgramError {
  if (typeof error === "number") {
    return {
      type: "Runtime",
      message: TRANSACTION_ERROR_NAME[error] ?? `TransactionError(${error})`,
    };
  }
  if (typeof error.err === "function") {
    return mapInstructionError(error.err());
  }
  // DuplicateInstruction / InsufficientFundsForRent / ...: not reachable by the
  // instruction-level path, surfaced as stable runtime errors.
  const label = (error as { constructor?: { name?: string } }).constructor?.name;
  return { type: "Runtime", message: label ?? "TransactionError" };
}

function mapInstructionError(error: InstructionErrorValue): ProgramError {
  if (typeof error === "number") {
    const named = NAMED_INSTRUCTION_ERROR[error];
    if (named !== undefined) return { type: named } as ProgramError;
    return {
      type: "Runtime",
      message: INSTRUCTION_ERROR_NAME[error] ?? `InstructionError(${error})`,
    };
  }
  if (typeof error.code === "number") {
    return { type: "Custom", code: error.code };
  }
  if (typeof error.msg === "string") {
    return { type: "BorshIoError" };
  }
  return { type: "Runtime", message: "InstructionError" };
}
