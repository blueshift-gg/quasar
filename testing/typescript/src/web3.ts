import {
  createKeyedAssociatedTokenAccount,
  createKeyedMintAccount,
  createKeyedSystemAccount,
  createKeyedTokenAccount,
  LAMPORTS_PER_SOL,
  QuasarSvm,
  type ExecutionResult,
  type KeyedAccountInfo,
  type QuasarSvmConfig,
} from "@blueshift-gg/quasar-svm/web3.js";
import { getMintDecoder, getTokenDecoder } from "@solana-program/token";
import { Address, Keypair, type TransactionInstruction } from "@solana/web3.js";
import { readFile } from "node:fs/promises";
import { QuasarTestResult } from "./result.js";

export { QuasarTestResult } from "./result.js";

export const DEFAULT_ACTOR_LAMPORTS = 10n * LAMPORTS_PER_SOL;
const SYSTEM_PROGRAM = new Address("11111111111111111111111111111111");

/** Persistent test world for the web3.js QuasarSVM adapter. */
export class QuasarTest {
  readonly svm: QuasarSvm;
  private readonly accounts = new Map<string, KeyedAccountInfo>();

  constructor(
    readonly programId: Address,
    elf: Uint8Array,
    config?: QuasarSvmConfig,
  ) {
    this.svm = new QuasarSvm(config).addProgram(programId, elf);
  }

  static async load(
    programId: Address,
    programPath = process.env.QUASAR_PROGRAM_PATH,
    config?: QuasarSvmConfig,
  ): Promise<QuasarTest> {
    if (!programPath) {
      throw new Error(
        "QUASAR_PROGRAM_PATH is not set; run through `quasar test` or pass an artifact path",
      );
    }
    return new QuasarTest(programId, await readFile(programPath), config);
  }

  async actor(lamports = DEFAULT_ACTOR_LAMPORTS): Promise<Address> {
    const { publicKey } = await Keypair.generate();
    return this.fund(publicKey, lamports);
  }

  async actors(count: number): Promise<Address[]> {
    return Promise.all(Array.from({ length: count }, () => this.actor()));
  }

  actorAt(address: Address): Address {
    return this.fund(address, DEFAULT_ACTOR_LAMPORTS);
  }

  fund(address: Address, lamports: bigint): Address {
    this.setAccount(createKeyedSystemAccount(address, lamports));
    return address;
  }

  empty(address: Address): Address {
    return this.fund(address, 0n);
  }

  async mint(authority: Address): Promise<Address> {
    return this.mintWithSupply(authority, 0n);
  }

  async mintWithSupply(authority: Address, supply: bigint): Promise<Address> {
    const { publicKey } = await Keypair.generate();
    return this.mintAt(publicKey, authority, supply, 6);
  }

  mintAt(
    address: Address,
    authority: Address,
    supply: bigint,
    decimals: number,
  ): Address {
    this.setAccount(
      createKeyedMintAccount(address, { decimals, mintAuthority: authority, supply }),
    );
    return address;
  }

  async tokenAccount(owner: Address, mint: Address, amount: bigint): Promise<Address> {
    const { publicKey } = await Keypair.generate();
    return this.tokenAccountAt(publicKey, owner, mint, amount);
  }

  tokenAccountAt(
    address: Address,
    owner: Address,
    mint: Address,
    amount: bigint,
  ): Address {
    this.setAccount(createKeyedTokenAccount(address, { amount, mint, owner }));
    return address;
  }

  ata(owner: Address, mint: Address, amount: bigint): Address {
    const account = createKeyedAssociatedTokenAccount(owner, mint, amount);
    this.setAccount(account);
    return account.accountId;
  }

  setAccount(account: KeyedAccountInfo): this {
    this.accounts.set(account.accountId.toBase58(), account);
    return this;
  }

  account(address: Address): KeyedAccountInfo | null {
    return this.accounts.get(address.toBase58()) ?? null;
  }

  async send(
    instruction: TransactionInstruction | Promise<TransactionInstruction>,
  ): Promise<QuasarTestResult<Address, KeyedAccountInfo, ExecutionResult>> {
    const result = this.svm.processInstruction(await instruction, [...this.accounts.values()]);
    this.commit(result);
    return this.result(result);
  }

  async sendWith(
    instruction: TransactionInstruction | Promise<TransactionInstruction>,
    accounts: Iterable<KeyedAccountInfo>,
  ): Promise<QuasarTestResult<Address, KeyedAccountInfo, ExecutionResult>> {
    const result = this.svm.processInstruction(
      await instruction,
      this.executionAccounts(accounts),
    );
    this.commit(result);
    return this.result(result);
  }

  async simulate(
    instruction: TransactionInstruction | Promise<TransactionInstruction>,
  ): Promise<QuasarTestResult<Address, KeyedAccountInfo, ExecutionResult>> {
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

  private executionAccounts(extra: Iterable<KeyedAccountInfo>): KeyedAccountInfo[] {
    const accounts = new Map(this.accounts);
    for (const account of extra) accounts.set(account.accountId.toBase58(), account);
    return [...accounts.values()];
  }

  private result(
    raw: ExecutionResult,
  ): QuasarTestResult<Address, KeyedAccountInfo, ExecutionResult> {
    return new QuasarTestResult(raw, {
      // QuasarSVM 0.1.x compares web3.js Address objects by identity. Results
      // contain freshly decoded instances, so compare their bytes instead.
      account: (address) =>
        raw.accounts.find((account) => account.accountId.equals(address)) ?? null,
      isClosed: (account) =>
        account.accountInfo.lamports === 0n &&
        account.accountInfo.data.length === 0 &&
        account.accountInfo.owner.equals(SYSTEM_PROGRAM),
      lamports: (account) => account.accountInfo.lamports,
      mintSupply: (account) => getMintDecoder().decode(account.accountInfo.data).supply,
      renderAddress: (address) => address.toBase58(),
      tokenBalance: (account) => getTokenDecoder().decode(account.accountInfo.data).amount,
    });
  }
}
