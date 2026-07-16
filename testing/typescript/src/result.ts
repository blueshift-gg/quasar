export interface RawExecutionResult {
  readonly computeUnits: bigint;
  readonly logs: readonly string[];
  readonly status: { readonly ok: boolean };
  assertCustomError(code: number): void;
  assertSuccess(): void;
}

interface ResultAdapter<Address, Account> {
  account(address: Address): Account | null;
  lamports(account: Account): bigint;
  mintSupply(account: Account): bigint;
  tokenBalance(account: Account): bigint;
  isClosed(account: Account): boolean;
  renderAddress(address: Address): string;
}

/** Fluent assertions over a committed QuasarSVM execution. */
export class QuasarTestResult<Address, Account, Raw extends RawExecutionResult> {
  constructor(
    readonly raw: Raw,
    private readonly adapter: ResultAdapter<Address, Account>,
  ) {}

  succeeds(): this {
    this.raw.assertSuccess();
    return this;
  }

  failsWith(code: number): this {
    this.raw.assertCustomError(code);
    return this;
  }

  cuBelow(limit: number | bigint): this {
    const expected = BigInt(limit);
    if (this.raw.computeUnits >= expected) {
      throw new Error(
        `expected fewer than ${expected} compute units, consumed ${this.raw.computeUnits}`,
      );
    }
    return this;
  }

  hasLamports(address: Address, expected: bigint): this {
    const account = this.requireAccount(address);
    this.expectEqual("lamport balance", address, this.adapter.lamports(account), expected);
    return this;
  }

  hasTokens(address: Address, expected: bigint): this {
    const account = this.requireAccount(address);
    this.expectEqual("token balance", address, this.adapter.tokenBalance(account), expected);
    return this;
  }

  hasSupply(address: Address, expected: bigint): this {
    const account = this.requireAccount(address);
    this.expectEqual("mint supply", address, this.adapter.mintSupply(account), expected);
    return this;
  }

  isClosed(address: Address): this {
    const account = this.adapter.account(address);
    if (account !== null && !this.adapter.isClosed(account)) {
      throw new Error(`expected ${this.adapter.renderAddress(address)} to be closed`);
    }
    return this;
  }

  account(address: Address): Account | null {
    return this.adapter.account(address);
  }

  private requireAccount(address: Address): Account {
    const account = this.adapter.account(address);
    if (account === null) {
      throw new Error(`execution result does not contain ${this.adapter.renderAddress(address)}`);
    }
    return account;
  }

  private expectEqual(
    label: string,
    address: Address,
    actual: bigint,
    expected: bigint,
  ): void {
    if (actual !== expected) {
      throw new Error(
        `unexpected ${label} for ${this.adapter.renderAddress(address)}: expected ${expected}, got ${actual}`,
      );
    }
  }
}
