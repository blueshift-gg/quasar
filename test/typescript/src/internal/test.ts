import type { Fixture, TokenProgram } from "./fixture.js";
import type {
  AccountChange,
  OutcomeAdapter,
  RawExecutionResult,
} from "./outcome.js";

type FixtureValue<Value> = Value extends Fixture<infer Output, infer _Host>
  ? Awaited<Output>
  : never;

type Installed<Input> = Input extends readonly unknown[]
  ? { [Index in keyof Input]: FixtureValue<Input[Index]> }
  : FixtureValue<Input>;

interface InstructionAccount<Address> {
  address: Address;
  writable: boolean;
}

/** Stable runtime limits accepted by both TypeScript test adapters. */
export interface TestOptions {
  /** Maximum compute units available to one transaction. */
  readonly computeUnitLimit?: bigint;
}

export interface HarnessResult<Account> extends RawExecutionResult {
  readonly accounts: readonly Account[];
}

export interface HarnessRuntime<Address, Account, Instruction> {
  addProgram(programId: Address, elf: Uint8Array, loaderVersion?: number): unknown;
  processInstructionChain(
    instructions: Instruction[],
    accounts: Account[],
  ): HarnessResult<Account>;
  simulateInstructionChain(
    instructions: Instruction[],
    accounts: Account[],
  ): HarnessResult<Account>;
  setComputeBudget(maxUnits: bigint): void;
  warpToTimestamp(timestamp: bigint): void;
  free(): void;
}

export interface HarnessAdapter<Address, Account, Instruction, Output>
  extends OutcomeAdapter<Address, Account> {
  freshAddress(bytes: Uint8Array): Address;
  accountLamports(account: Account): bigint;
  instructionAccounts(instruction: Instruction): readonly InstructionAccount<Address>[];
  emptyAccount(address: Address): Account;
  accountsEqual(left: Account | null, right: Account | null): boolean;
  deriveAta(owner: Address, mint: Address, tokenProgram: TokenProgram): Promise<Address>;
  deriveProgramAddress(
    seeds: readonly Uint8Array[],
    programId: Address,
  ): Promise<readonly [Address, number]>;
  outcome(
    result: HarnessResult<Account>,
    accounts: readonly Account[],
    changes: readonly AccountChange<Address, Account>[],
  ): Output;
}

export class TestCore<Address, Account, Instruction, Output> {
  readonly #runtime: HarnessRuntime<Address, Account, Instruction>;
  readonly #adapter: HarnessAdapter<Address, Account, Instruction, Output>;
  readonly #accounts = new Map<string, Account>();
  readonly #primaryProgramId: Address | undefined;
  #freshAddresses = 0n;

  protected constructor(
    runtime: HarnessRuntime<Address, Account, Instruction>,
    adapter: HarnessAdapter<Address, Account, Instruction, Output>,
    programId?: Address,
    elf?: Uint8Array,
    options: TestOptions = {},
  ) {
    if ((programId === undefined) !== (elf === undefined)) {
      throw new Error("Test needs both a program address and ELF, or neither");
    }
    this.#runtime = runtime;
    this.#adapter = adapter;
    this.#primaryProgramId = programId;
    if (options.computeUnitLimit !== undefined) {
      if (
        options.computeUnitLimit < 0n ||
        options.computeUnitLimit > 0xffff_ffff_ffff_ffffn
      ) {
        throw new Error("computeUnitLimit must fit a u64");
      }
      runtime.setComputeBudget(options.computeUnitLimit);
    }
    if (programId !== undefined && elf !== undefined) {
      this.loadProgram(programId, elf);
    }
  }

  get programId(): Address {
    if (this.#primaryProgramId === undefined) {
      throw new Error("this Test has no primary program");
    }
    return this.#primaryProgramId;
  }

  async add<
    const Input extends
      | Fixture<unknown, this>
      | readonly Fixture<unknown, this>[],
  >(input: Input): Promise<Installed<Input>> {
    if (!Array.isArray(input)) {
      return (await (input as Fixture<unknown, this>).install(this)) as Installed<Input>;
    }

    const installed: unknown[] = [];
    for (const fixture of input as readonly Fixture<unknown, this>[]) {
      installed.push(await fixture.install(this));
    }
    return installed as Installed<Input>;
  }

  setAccount(account: Account): void {
    this.#accounts.set(
      this.#adapter.addressKey(this.#adapter.accountAddress(account)),
      account,
    );
  }

  account(address: Address): Account | null {
    return this.#accounts.get(this.#adapter.addressKey(address)) ?? null;
  }

  accountAs<Value>(
    address: Address,
    decode: (data: Uint8Array) => Value,
  ): Value | null {
    const account = this.account(address);
    return account === null
      ? null
      : decode(this.#adapter.accountData(account));
  }

  loadProgram(programId: Address, elf: Uint8Array, loaderVersion?: number): void {
    this.#runtime.addProgram(programId, elf, loaderVersion);
  }

  freshAddress(): Address {
    this.#freshAddresses += 1n;
    const bytes = new Uint8Array(32);
    bytes.set(new TextEncoder().encode("quasar-test/fresh-address"));
    new DataView(bytes.buffer).setBigUint64(24, this.#freshAddresses, true);
    return this.#adapter.freshAddress(bytes);
  }

  deriveAta(
    owner: Address,
    mint: Address,
    tokenProgram: TokenProgram = "legacy",
  ): Promise<Address> {
    return this.#adapter.deriveAta(owner, mint, tokenProgram);
  }

  async derivePda(seeds: readonly Uint8Array[]): Promise<Address> {
    return (await this.derivePdaWithBump(seeds))[0];
  }

  derivePdaWithBump(
    seeds: readonly Uint8Array[],
  ): Promise<readonly [Address, number]> {
    return this.#adapter.deriveProgramAddress(seeds, this.programId);
  }

  lamports(address: Address): bigint {
    return this.#adapter.accountLamports(this.#requiredAccount(address));
  }

  tokens(address: Address): bigint {
    return this.#adapter.tokenAmount(this.#requiredAccount(address));
  }

  supply(address: Address): bigint {
    return this.#adapter.mintSupply(this.#requiredAccount(address));
  }

  warpToTimestamp(timestamp: bigint): void {
    if (
      timestamp < -0x8000_0000_0000_0000n ||
      timestamp > 0x7fff_ffff_ffff_ffffn
    ) {
      throw new Error("timestamp must fit an i64");
    }
    this.#runtime.warpToTimestamp(timestamp);
  }

  send(instruction: Instruction): Output {
    return this.#execute([instruction], [], true);
  }

  sendAll(instructions: readonly Instruction[]): Output {
    return this.#execute([...instructions], [], true);
  }

  sendWith(instruction: Instruction, accounts: readonly Account[]): Output {
    return this.#execute([instruction], [...accounts], true);
  }

  simulate(instruction: Instruction): Output {
    return this.#execute([instruction], [], false);
  }

  free(): void {
    this.#runtime.free();
  }

  [Symbol.dispose](): void {
    this.free();
  }

  #execute(
    instructions: Instruction[],
    explicitAccounts: Account[],
    commit: boolean,
  ): Output {
    if (instructions.length === 0) {
      throw new Error("a transaction needs an instruction");
    }

    const inputs = new Map<string, Account>();
    for (const account of explicitAccounts) {
      const address = this.#adapter.accountAddress(account);
      const key = this.#adapter.addressKey(address);
      if (inputs.has(key)) {
        throw new Error(`transaction input contains account ${key} more than once`);
      }
      inputs.set(key, account);
    }

    const tracked = new Map<string, InstructionAccount<Address>>();
    for (const instruction of instructions) {
      for (const meta of this.#adapter.instructionAccounts(instruction)) {
        const key = this.#adapter.addressKey(meta.address);
        const previous = tracked.get(key);
        tracked.set(key, {
          address: meta.address,
          writable: meta.writable || previous?.writable === true,
        });
      }
    }

    for (const [key, account] of inputs) {
      if (tracked.has(key)) continue;
      tracked.set(key, {
        address: this.#adapter.accountAddress(account),
        writable: false,
      });
    }

    const before = new Map<string, Account | null>();
    for (const [key, meta] of tracked) {
      const account = inputs.get(key) ?? this.#accounts.get(key) ?? null;
      before.set(key, account);
      if (!inputs.has(key)) {
        if (account !== null) {
          inputs.set(key, account);
        } else if (meta.writable) {
          inputs.set(key, this.#adapter.emptyAccount(meta.address));
        }
      }
    }

    const result = commit
      ? this.#runtime.processInstructionChain(instructions, [...inputs.values()])
      : this.#runtime.simulateInstructionChain(instructions, [...inputs.values()]);

    const succeeded = result.status.ok;
    const resultingAccounts = result.accounts;
    if (commit && succeeded) {
      for (const account of resultingAccounts) {
        this.setAccount(account);
      }
    }

    const returned = new Map(
      resultingAccounts.map(account => [
        this.#adapter.addressKey(this.#adapter.accountAddress(account)),
        account,
      ]),
    );
    const outcomeAccounts: Account[] = [];
    const changes: AccountChange<Address, Account>[] = [];
    for (const [key, meta] of tracked) {
      const previous = before.get(key) ?? null;
      const next = succeeded ? (returned.get(key) ?? null) : previous;
      if (next !== null) outcomeAccounts.push(next);
      if (
        succeeded &&
        meta.writable &&
        !this.#adapter.accountsEqual(previous, next)
      ) {
        changes.push({
          address: meta.address,
          before: previous,
          after: next,
        });
      }
    }
    return this.#adapter.outcome(result, outcomeAccounts, changes);
  }

  #requiredAccount(address: Address): Account {
    const account = this.account(address);
    if (account === null) {
      throw new Error(`no account at ${this.#adapter.addressKey(address)}`);
    }
    return account;
  }
}
