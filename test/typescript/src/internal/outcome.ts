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
  lamports(account: Account): bigint;
  mintSupply(account: Account): bigint;
  tokenAmount(account: Account): bigint;
  isClosed(account: Account): boolean;
  renderAddress(address: Address): string;
}

export interface AccountChange<Address, Account> {
  readonly address: Address;
  readonly before: Account | null;
  readonly after: Account | null;
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
