import type { Fixture, TokenProgram } from "./fixture.js";
import type { AccountChange } from "./outcome.js";

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

export interface HarnessRuntime<Address, Account, Instruction, Result> {
  addProgram(programId: Address, elf: Uint8Array, loaderVersion?: number): unknown;
  processInstructionChain(instructions: Instruction[], accounts: Account[]): Result;
  simulateInstructionChain(instructions: Instruction[], accounts: Account[]): Result;
  warpToTimestamp(timestamp: bigint): void;
  free(): void;
}

export interface HarnessAdapter<Address, Account, Instruction, Result, Output> {
  addressKey(address: Address): string;
  freshAddress(bytes: Uint8Array): Address;
  accountAddress(account: Account): Address;
  accountData(account: Account): Uint8Array;
  accountLamports(account: Account): bigint;
  instructionAccounts(instruction: Instruction): readonly InstructionAccount<Address>[];
  emptyAccount(address: Address): Account;
  resultAccounts(result: Result): readonly Account[];
  resultSucceeded(result: Result): boolean;
  tokenAmount(account: Account): bigint;
  mintSupply(account: Account): bigint;
  accountsEqual(left: Account | null, right: Account | null): boolean;
  deriveAta(owner: Address, mint: Address, tokenProgram: TokenProgram): Promise<Address>;
  deriveProgramAddress(
    seeds: readonly Uint8Array[],
    programId: Address,
  ): Promise<readonly [Address, number]>;
  outcome(
    result: Result,
    changes: readonly AccountChange<Address, Account>[],
  ): Output;
}

export class TestCore<Address, Account, Instruction, Result, Output> {
  readonly #runtime: HarnessRuntime<Address, Account, Instruction, Result>;
  readonly #adapter: HarnessAdapter<Address, Account, Instruction, Result, Output>;
  readonly #accounts = new Map<string, Account>();
  readonly #primaryProgramId: Address | undefined;
  #freshAddresses = 0n;

  protected constructor(
    runtime: HarnessRuntime<Address, Account, Instruction, Result>,
    adapter: HarnessAdapter<Address, Account, Instruction, Result, Output>,
    programId?: Address,
    elf?: Uint8Array,
  ) {
    if ((programId === undefined) !== (elf === undefined)) {
      throw new Error("Test needs both a program address and ELF, or neither");
    }
    this.#runtime = runtime;
    this.#adapter = adapter;
    this.#primaryProgramId = programId;
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

    for (const [key, meta] of tracked) {
      if (inputs.has(key)) continue;
      const account = this.#accounts.get(key);
      if (account !== undefined) {
        inputs.set(key, account);
      } else if (meta.writable) {
        inputs.set(key, this.#adapter.emptyAccount(meta.address));
      }
    }

    const before = new Map<string, Account | null>();
    for (const [key, meta] of tracked) {
      if (meta.writable) before.set(key, inputs.get(key) ?? null);
    }

    const result = commit
      ? this.#runtime.processInstructionChain(instructions, [...inputs.values()])
      : this.#runtime.simulateInstructionChain(instructions, [...inputs.values()]);

    const succeeded = this.#adapter.resultSucceeded(result);
    const resultingAccounts = this.#adapter.resultAccounts(result);
    if (commit && succeeded) {
      for (const account of resultingAccounts) {
        this.setAccount(account);
      }
    }

    const after = new Map(
      resultingAccounts.map(account => [
        this.#adapter.addressKey(this.#adapter.accountAddress(account)),
        account,
      ]),
    );
    const changes: AccountChange<Address, Account>[] = [];
    if (succeeded) {
      for (const [key, previous] of before) {
        const next = after.get(key) ?? null;
        if (!this.#adapter.accountsEqual(previous, next)) {
          changes.push({
            address: tracked.get(key)!.address,
            before: previous,
            after: next,
          });
        }
      }
    }
    return this.#adapter.outcome(result, changes);
  }

  #requiredAccount(address: Address): Account {
    const account = this.account(address);
    if (account === null) {
      throw new Error(`no account at ${this.#adapter.addressKey(address)}`);
    }
    return account;
  }
}
