import {
  createKeyedAssociatedTokenAccount,
  createKeyedMintAccount,
  createKeyedSystemAccount,
  createKeyedTokenAccount,
  LAMPORTS_PER_SOL,
  QuasarSvm,
  type ExecutionResult,
  type QuasarSvmConfig,
} from "@blueshift-gg/quasar-svm/kit";
import { getMintDecoder, getTokenDecoder } from "@solana-program/token";
import {
  address,
  generateKeyPairSigner,
  type Account,
  type Address,
  type Instruction,
} from "@solana/kit";
import { readFile } from "node:fs/promises";
import { QuasarTestResult } from "./result.js";

export { QuasarTestResult } from "./result.js";

export const DEFAULT_ACTOR_LAMPORTS = 10n * LAMPORTS_PER_SOL;
const SYSTEM_PROGRAM = address("11111111111111111111111111111111");
type WorldAccount = Account<Uint8Array>;

/** Persistent test world for the Solana Kit QuasarSVM adapter. */
export class QuasarTest {
  readonly svm: QuasarSvm;
  private readonly accounts = new Map<Address, WorldAccount>();

  constructor(
    readonly programAddress: Address,
    elf: Uint8Array,
    config?: QuasarSvmConfig,
  ) {
    this.svm = new QuasarSvm(config).addProgram(programAddress, elf);
  }

  static async load(
    programAddress: Address,
    programPath = process.env.QUASAR_PROGRAM_PATH,
    config?: QuasarSvmConfig,
  ): Promise<QuasarTest> {
    if (!programPath) {
      throw new Error(
        "QUASAR_PROGRAM_PATH is not set; run through `quasar test` or pass an artifact path",
      );
    }
    return new QuasarTest(programAddress, await readFile(programPath), config);
  }

  async actor(lamports = DEFAULT_ACTOR_LAMPORTS): Promise<Address> {
    const signer = await generateKeyPairSigner();
    return this.fund(signer.address, lamports);
  }

  async actors(count: number): Promise<Address[]> {
    return Promise.all(Array.from({ length: count }, () => this.actor()));
  }

  actorAt(accountAddress: Address): Address {
    return this.fund(accountAddress, DEFAULT_ACTOR_LAMPORTS);
  }

  fund(accountAddress: Address, lamports: bigint): Address {
    this.setAccount(createKeyedSystemAccount(accountAddress, lamports));
    return accountAddress;
  }

  empty(accountAddress: Address): Address {
    return this.fund(accountAddress, 0n);
  }

  async mint(authority: Address): Promise<Address> {
    return this.mintWithSupply(authority, 0n);
  }

  async mintWithSupply(authority: Address, supply: bigint): Promise<Address> {
    const signer = await generateKeyPairSigner();
    return this.mintAt(signer.address, authority, supply, 6);
  }

  mintAt(
    accountAddress: Address,
    authority: Address,
    supply: bigint,
    decimals: number,
  ): Address {
    this.setAccount(
      createKeyedMintAccount(accountAddress, {
        decimals,
        mintAuthority: authority,
        supply,
      }),
    );
    return accountAddress;
  }

  async tokenAccount(owner: Address, mint: Address, amount: bigint): Promise<Address> {
    const signer = await generateKeyPairSigner();
    return this.tokenAccountAt(signer.address, owner, mint, amount);
  }

  tokenAccountAt(
    accountAddress: Address,
    owner: Address,
    mint: Address,
    amount: bigint,
  ): Address {
    this.setAccount(createKeyedTokenAccount(accountAddress, { amount, mint, owner }));
    return accountAddress;
  }

  async ata(owner: Address, mint: Address, amount: bigint): Promise<Address> {
    const account = await createKeyedAssociatedTokenAccount(owner, mint, amount);
    this.setAccount(account);
    return account.address;
  }

  setAccount(account: WorldAccount): this {
    this.accounts.set(account.address, account);
    return this;
  }

  account(accountAddress: Address): WorldAccount | null {
    return this.accounts.get(accountAddress) ?? null;
  }

  async send(
    instruction: Instruction | Promise<Instruction>,
  ): Promise<QuasarTestResult<Address, WorldAccount, ExecutionResult>> {
    const result = this.svm.processInstruction(await instruction, [...this.accounts.values()]);
    this.commit(result);
    return this.result(result);
  }

  async sendWith(
    instruction: Instruction | Promise<Instruction>,
    accounts: Iterable<WorldAccount>,
  ): Promise<QuasarTestResult<Address, WorldAccount, ExecutionResult>> {
    const result = this.svm.processInstruction(
      await instruction,
      this.executionAccounts(accounts),
    );
    this.commit(result);
    return this.result(result);
  }

  async simulate(
    instruction: Instruction | Promise<Instruction>,
  ): Promise<QuasarTestResult<Address, WorldAccount, ExecutionResult>> {
    return this.result(
      this.svm.processInstruction(await instruction, [...this.accounts.values()]),
    );
  }

  free(): void {
    this.svm.free();
  }

  private commit(result: ExecutionResult): void {
    for (const account of result.accounts) this.setAccount(account);
  }

  private executionAccounts(extra: Iterable<WorldAccount>): WorldAccount[] {
    const accounts = new Map(this.accounts);
    for (const account of extra) accounts.set(account.address, account);
    return [...accounts.values()];
  }

  private result(
    raw: ExecutionResult,
  ): QuasarTestResult<Address, WorldAccount, ExecutionResult> {
    return new QuasarTestResult(raw, {
      account: (accountAddress) => raw.account(accountAddress),
      isClosed: (account) =>
        account.lamports === 0n &&
        account.data.length === 0 &&
        account.programAddress === SYSTEM_PROGRAM,
      lamports: (account) => account.lamports,
      mintSupply: (account) => getMintDecoder().decode(account.data).supply,
      renderAddress: (accountAddress) => accountAddress,
      tokenBalance: (account) => getTokenDecoder().decode(account.data).amount,
    });
  }
}
