import type { ProgramError } from "@blueshift-gg/quasar-svm";

export interface RawExecutionResult {
  readonly status:
    | { readonly ok: true }
    | { readonly ok: false; readonly error: ProgramError };
  readonly computeUnits: bigint;
  readonly logs: readonly string[];
  readonly returnData: Uint8Array;
}

export interface OutcomeAdapter<Address, Account> {
  account(address: Address): Account | null;
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
export class Outcome<
  Address,
  Account,
  Raw extends RawExecutionResult = RawExecutionResult,
> {
  constructor(
    readonly raw: Raw,
    private readonly adapter: OutcomeAdapter<Address, Account>,
    readonly accountChanges: readonly AccountChange<Address, Account>[] = [],
  ) {}

  get error(): ProgramError | null {
    return this.raw.status.ok ? null : this.raw.status.error;
  }

  get computeUnits(): bigint {
    return this.raw.computeUnits;
  }

  get logs(): readonly string[] {
    return this.raw.logs;
  }

  get returnData(): Uint8Array {
    return this.raw.returnData;
  }

  isOk(): boolean {
    return this.raw.status.ok;
  }

  isErr(): boolean {
    return !this.raw.status.ok;
  }

  succeeds(): this {
    if (!this.raw.status.ok) {
      throw new Error(
        `expected success, got ${JSON.stringify(this.raw.status.error)}${this.formattedLogs()}`,
      );
    }
    return this;
  }

  fails(expected: ProgramError): this {
    if (this.raw.status.ok) {
      throw new Error(
        `expected error ${JSON.stringify(expected)}, but execution succeeded`,
      );
    }
    if (JSON.stringify(this.raw.status.error) !== JSON.stringify(expected)) {
      throw new Error(
        `expected error ${JSON.stringify(expected)}, got ${JSON.stringify(this.raw.status.error)}${this.formattedLogs()}`,
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
    return this.adapter.account(address);
  }

  accountAs<Value>(
    address: Address,
    decode: (data: Uint8Array) => Value,
  ): Value | null {
    const account = this.account(address);
    return account === null ? null : decode(this.adapter.accountData(account));
  }

  returnValue<Value>(decode: (data: Uint8Array) => Value): Value {
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
