/** Stable execution errors exposed by the test harness. */
export type ProgramError =
  | { readonly type: "InvalidArgument" }
  | { readonly type: "InvalidInstructionData" }
  | { readonly type: "InvalidAccountData" }
  | { readonly type: "AccountDataTooSmall" }
  | { readonly type: "InsufficientFunds" }
  | { readonly type: "IncorrectProgramId" }
  | { readonly type: "MissingRequiredSignature" }
  | { readonly type: "AccountAlreadyInitialized" }
  | { readonly type: "UninitializedAccount" }
  | { readonly type: "MissingAccount" }
  | { readonly type: "InvalidSeeds" }
  | { readonly type: "ArithmeticOverflow" }
  | { readonly type: "AccountNotRentExempt" }
  | { readonly type: "InvalidAccountOwner" }
  | { readonly type: "IncorrectAuthority" }
  | { readonly type: "Immutable" }
  | { readonly type: "BorshIoError" }
  | { readonly type: "ComputeBudgetExceeded" }
  | { readonly type: "Custom"; readonly code: number }
  | { readonly type: "Runtime"; readonly message: string };

export interface RawExecutionResult {
  readonly status:
    | { readonly ok: true }
    | { readonly ok: false; readonly error: ProgramError };
  readonly computeUnits: bigint;
  readonly logs: readonly string[];
  readonly returnData: Uint8Array;
}

export interface OutcomeAdapter<Address, Account> {
  addressKey(address: Address): string;
  accountAddress(account: Account): Address;
  accountData(account: Account): Uint8Array;
  accountOwner(account: Account): Address;
  lamports(account: Account): bigint;
  mintSupply(account: Account): bigint;
  tokenAmount(account: Account): bigint;
  isClosed(account: Account): boolean;
  renderAddress(address: Address): string;
}

/**
 * A typed view over a Quasar account's raw bytes.
 *
 * `decode`/`encode` operate on the account body — the bytes *after* the
 * discriminator when one is present. The harness owns the framing metadata:
 * `read`/`hasState` validate `owner`, `discriminator`, and `size` against the
 * raw account before handing the stripped body to `decode`, and `write` frames
 * an encoded body with the discriminator. This mirrors the Rust `read`/`write`
 * semantics and pairs directly with a generated client's `XCodec`,
 * `X_DISCRIMINATOR`, and `PROGRAM_ADDRESS` exports.
 */
export interface AccountCodec<Value, Address> {
  /** Decode the account body (post-discriminator bytes) into a typed value. */
  decode(bytes: Uint8Array): Value;
  /** Encode a typed value into the account body, for `write`. */
  encode?(value: Value): ArrayLike<number>;
  /** Program expected to own the account; validated by `read`/`hasState`. */
  owner?: Address;
  /** Leading discriminator bytes; validated and stripped before `decode`. */
  discriminator?: Uint8Array;
  /** Minimum expected raw account length in bytes, discriminator included. */
  size?: number;
}

function describeBytes(bytes: Uint8Array): string {
  return `[${Array.from(bytes).join(", ")}]`;
}

/**
 * Validate a raw account against a codec's framing metadata and decode its
 * body. Shared by `Test.read` and `Outcome.hasState`; throws with a precise
 * message on any owner, discriminator, or size mismatch.
 */
export function decodeAccount<Value, Address, Account>(
  codec: AccountCodec<Value, Address>,
  address: Address,
  account: Account,
  adapter: Pick<
    OutcomeAdapter<Address, Account>,
    "accountData" | "accountOwner" | "addressKey" | "renderAddress"
  >,
): Value {
  const data = adapter.accountData(account);
  if (codec.owner !== undefined) {
    const owner = adapter.accountOwner(account);
    if (adapter.addressKey(owner) !== adapter.addressKey(codec.owner)) {
      throw new Error(
        `account ${adapter.renderAddress(address)} is owned by ${adapter.renderAddress(owner)}, expected ${adapter.renderAddress(codec.owner)}`,
      );
    }
  }
  const discriminator = codec.discriminator;
  if (discriminator !== undefined) {
    if (
      data.length < discriminator.length ||
      !discriminator.every((byte, index) => data[index] === byte)
    ) {
      throw new Error(
        `account ${adapter.renderAddress(address)} has discriminator ${describeBytes(data.subarray(0, discriminator.length))}, expected ${describeBytes(discriminator)}`,
      );
    }
  }
  if (codec.size !== undefined && data.length < codec.size) {
    throw new Error(
      `account ${adapter.renderAddress(address)} holds ${data.length} bytes, expected at least ${codec.size}`,
    );
  }
  const body =
    discriminator === undefined ? data : data.subarray(discriminator.length);
  return codec.decode(body);
}

/** A writable account's state before and after an execution. */
export class AccountChange<Address, Account> {
  constructor(
    readonly address: Address,
    /** State before execution, or `null` when the execution created it. */
    readonly before: Account | null,
    /** State after execution, or `null` when the execution removed it. */
    readonly after: Account | null,
  ) {}

  /** Whether this account did not exist before execution. */
  wasCreated(): boolean {
    return this.before === null && this.after !== null;
  }

  /** Whether this account no longer exists after execution. */
  wasRemoved(): boolean {
    return this.before !== null && this.after === null;
  }
}

/** Structured execution assertions independent of the SVM adapter in use. */
export class Outcome<Address, Account> {
  readonly #error: ProgramError | null;
  readonly #accounts: ReadonlyMap<string, Account>;
  readonly computeUnits: bigint;
  readonly logs: readonly string[];
  readonly returnData: Uint8Array;

  constructor(
    result: RawExecutionResult,
    accounts: readonly Account[],
    private readonly adapter: OutcomeAdapter<Address, Account>,
    readonly accountChanges: readonly AccountChange<Address, Account>[] = [],
  ) {
    this.#error = result.status.ok ? null : result.status.error;
    this.#accounts = new Map(
      accounts.map(account => [
        adapter.addressKey(adapter.accountAddress(account)),
        account,
      ]),
    );
    this.computeUnits = result.computeUnits;
    this.logs = [...result.logs];
    this.returnData = result.returnData.slice();
  }

  get error(): ProgramError | null {
    return this.#error;
  }

  isOk(): boolean {
    return this.#error === null;
  }

  isErr(): boolean {
    return this.#error !== null;
  }

  succeeds(): this {
    if (this.#error !== null) {
      throw new Error(
        `expected success, got ${JSON.stringify(this.#error)}${this.formattedLogs()}`,
      );
    }
    return this;
  }

  fails(expected: ProgramError): this {
    if (this.#error === null) {
      throw new Error(
        `expected error ${JSON.stringify(expected)}, but execution succeeded`,
      );
    }
    if (!errorsEqual(this.#error, expected)) {
      throw new Error(
        `expected error ${JSON.stringify(expected)}, got ${JSON.stringify(this.#error)}${this.formattedLogs()}`,
      );
    }
    return this;
  }

  failsWith(code: number): this {
    return this.fails({ type: "Custom", code });
  }

  cuAtMost(limit: bigint | number): this {
    const ceiling = BigInt(limit);
    if (this.computeUnits > ceiling) {
      throw new Error(
        `expected at most ${ceiling} compute units, consumed ${this.computeUnits}`,
      );
    }
    return this;
  }

  account(address: Address): Account | null {
    return this.#accounts.get(this.adapter.addressKey(address)) ?? null;
  }

  accountAs<Value>(
    address: Address,
    decode: (data: Uint8Array) => Value,
  ): Value | null {
    const account = this.account(address);
    return account === null ? null : decode(this.adapter.accountData(account));
  }

  /**
   * Decode a resulting account through a typed codec and run assertions against
   * it. Validates owner, discriminator, and size like `Test.read`, throwing on
   * mismatch; the closure asserts on the decoded state. Chainable.
   */
  hasState<Value>(
    codec: AccountCodec<Value, Address>,
    address: Address,
    check: (state: Value) => void,
  ): this {
    const account = this.account(address);
    if (account === null) {
      throw new Error(
        `outcome does not contain account ${this.adapter.renderAddress(address)}`,
      );
    }
    check(decodeAccount(codec, address, account, this.adapter));
    return this;
  }

  returnValue<Value>(decode: (data: Uint8Array) => Value | null): Value | null {
    return decode(this.returnData);
  }

  events<Value>(decode: (data: Uint8Array) => Value | null): Value[] {
    const values: Value[] = [];
    for (const log of this.logs) {
      if (!log.startsWith("Program data: ")) continue;
      try {
        const value = decode(
          Buffer.from(log.slice("Program data: ".length), "base64"),
        );
        if (value !== null) values.push(value);
      } catch {
        // A transaction may contain unrelated or malformed program-data logs.
      }
    }
    return values;
  }

  hasLamports(address: Address, expected: bigint): this {
    return this.expectAccountValue(
      "lamport balance",
      address,
      expected,
      account => this.adapter.lamports(account),
    );
  }

  hasTokens(address: Address, expected: bigint): this {
    return this.expectAccountValue(
      "token balance",
      address,
      expected,
      account => this.adapter.tokenAmount(account),
    );
  }

  hasSupply(address: Address, expected: bigint): this {
    return this.expectAccountValue(
      "mint supply",
      address,
      expected,
      account => this.adapter.mintSupply(account),
    );
  }

  isClosed(address: Address): this {
    const account = this.account(address);
    if (account !== null && !this.adapter.isClosed(account)) {
      throw new Error(
        `account ${this.adapter.renderAddress(address)} is not closed`,
      );
    }
    return this;
  }

  private expectAccountValue(
    label: string,
    address: Address,
    expected: bigint,
    read: (account: Account) => bigint,
  ): this {
    const account = this.requiredAccount(address);
    const actual = read(account);
    if (actual !== expected) {
      throw new Error(
        `unexpected ${label} for ${this.adapter.renderAddress(address)}: expected ${expected}, got ${actual}`,
      );
    }
    return this;
  }

  private requiredAccount(address: Address): Account {
    const account = this.account(address);
    if (account === null) {
      throw new Error(
        `outcome does not contain account ${this.adapter.renderAddress(address)}`,
      );
    }
    return account;
  }

  private formattedLogs(): string {
    return this.logs.length === 0
      ? ""
      : `\nprogram logs:\n  ${this.logs.join("\n  ")}`;
  }
}

function errorsEqual(left: ProgramError, right: ProgramError): boolean {
  if (left.type !== right.type) return false;
  if (left.type === "Custom" && right.type === "Custom") {
    return left.code === right.code;
  }
  if (left.type === "Runtime" && right.type === "Runtime") {
    return left.message === right.message;
  }
  return true;
}
