import {
  createKeyedMintAccount,
  createKeyedSystemAccount,
  createKeyedTokenAccount,
  QuasarSvm,
  SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
  SPL_TOKEN_2022_PROGRAM_ID,
  SPL_TOKEN_PROGRAM_ID,
} from "@blueshift-gg/quasar-svm/web3.js";
import { getMintDecoder, getTokenDecoder } from "@solana-program/token";
import {
  Address,
  type KeyedAccountInfo,
  type TransactionInstruction,
} from "@solana/web3.js";
import { readFile } from "node:fs/promises";
import {
  createFixtureFactories,
  DEFAULT_WALLET_LAMPORTS,
  TokenProgram,
  type AccountOptions as SharedAccountOptions,
  type AssociatedTokenAccountOptions,
  type Fixture as SharedFixture,
  type MintHolder as SharedMintHolder,
  type MintOptions as SharedMintOptions,
  type TokenAccountOptions as SharedTokenAccountOptions,
  type WalletOptions as SharedWalletOptions,
} from "./internal/fixture.js";
import {
  Outcome as SharedOutcome,
  type AccountChange as SharedAccountChange,
  type AccountCodec as SharedAccountCodec,
} from "./internal/outcome.js";
import {
  TestCore,
  type HarnessAdapter,
  type TestOptions as SharedTestOptions,
} from "./internal/test.js";

export { DEFAULT_WALLET_LAMPORTS, TokenProgram };
export type { ProgramError } from "./internal/outcome.js";
export type { AssociatedTokenAccountOptions };

export type Fixture<Output> = SharedFixture<Output, Test>;
export type Outcome = SharedOutcome<Address, KeyedAccountInfo>;
export type AccountChange = SharedAccountChange<Address, KeyedAccountInfo>;
export type AccountCodec<Value> = SharedAccountCodec<Value, Address>;
export type WalletOptions = SharedWalletOptions<Address>;
export type MintOptions = SharedMintOptions<Address>;
export type MintHolder = SharedMintHolder<Address>;
export type TokenAccountOptions = SharedTokenAccountOptions<Address>;
export type AccountOptions = SharedAccountOptions<Address>;
export type TestOptions = SharedTestOptions;

/** Account metas for read-only co-signers, e.g. multisig signers. */
export function coSigners(
  addresses: readonly Address[],
): { pubkey: Address; isSigner: boolean; isWritable: boolean }[] {
  return addresses.map(pubkey => ({
    pubkey,
    isSigner: true,
    isWritable: false,
  }));
}

/** Value-equality for addresses, independent of the backend representation. */
export function addressesEqual(left: Address, right: Address): boolean {
  return left.equals(right);
}

const systemProgram = new Address("11111111111111111111111111111111");

function bytesEqual(left: Uint8Array, right: Uint8Array): boolean {
  return (
    left.length === right.length &&
    left.every((byte, index) => byte === right[index])
  );
}

function programAccount(
  value: Address,
  owner: Address,
  data: Uint8Array,
  lamps: bigint,
): KeyedAccountInfo {
  return {
    accountId: value,
    accountInfo: {
      data,
      executable: false,
      lamports: lamps,
      owner,
      rentEpoch: 0n,
      space: BigInt(data.length),
    },
  };
}

const adapter: HarnessAdapter<
  Address,
  KeyedAccountInfo,
  TransactionInstruction,
  Outcome
> = {
  addressKey: value => value.toBase58(),
  freshAddress: bytes => new Address(bytes),
  accountAddress: account => account.accountId,
  accountData: account => account.accountInfo.data,
  accountOwner: account => account.accountInfo.owner,
  accountLamports: account => account.accountInfo.lamports,
  instructionAccounts: instruction =>
    instruction.keys.map(meta => ({
      address: meta.pubkey,
      writable: meta.isWritable,
      signer: meta.isSigner,
    })),
  emptyAccount: value => createKeyedSystemAccount(value, 0n),
  fundedAccount: value =>
    createKeyedSystemAccount(value, DEFAULT_WALLET_LAMPORTS),
  programAccount,
  tokenAmount: account =>
    BigInt(getTokenDecoder().decode(account.accountInfo.data).amount),
  mintSupply: account =>
    BigInt(getMintDecoder().decode(account.accountInfo.data).supply),
  accountsEqual: (left, right) =>
    left === null
      ? right === null
      : right !== null &&
        left.accountId.equals(right.accountId) &&
        left.accountInfo.owner.equals(right.accountInfo.owner) &&
        left.accountInfo.lamports === right.accountInfo.lamports &&
        left.accountInfo.executable === right.accountInfo.executable &&
        bytesEqual(left.accountInfo.data, right.accountInfo.data),
  async deriveAta(owner, mint, tokenProgram) {
    return (await Address.findProgramAddress(
      [
        owner.toBytes(),
        new Address(
          tokenProgram === TokenProgram.Token2022
            ? SPL_TOKEN_2022_PROGRAM_ID
            : SPL_TOKEN_PROGRAM_ID,
        ).toBytes(),
        mint.toBytes(),
      ],
      new Address(SPL_ASSOCIATED_TOKEN_PROGRAM_ID),
    ))[0];
  },
  deriveProgramAddress: (seeds, programId) =>
    Address.findProgramAddress([...seeds], programId),
  outcome: (raw, accounts, changes) =>
    new SharedOutcome(raw, accounts, adapter, changes),
  isClosed: account =>
    account.accountInfo.lamports === 0n &&
    account.accountInfo.data.length === 0 &&
    account.accountInfo.owner.equals(systemProgram),
  lamports: account => account.accountInfo.lamports,
  renderAddress: value => value.toBase58(),
};

/** An isolated fixture-first test world using Web3.js address and account types. */
export class Test extends TestCore<
  Address,
  KeyedAccountInfo,
  TransactionInstruction,
  Outcome
> {
  constructor(
    programId?: Address,
    elf?: Uint8Array,
    options: TestOptions = {},
  ) {
    super(new QuasarSvm(), adapter, programId, elf, options);
  }

  static async load(
    programId: Address,
    programPath = process.env.QUASAR_PROGRAM_PATH,
    options?: TestOptions,
  ): Promise<Test> {
    if (!programPath) {
      throw new Error(
        "QUASAR_PROGRAM_PATH is not set; run through `quasar test` or pass an artifact path",
      );
    }
    return new Test(programId, await readFile(programPath), options);
  }
}

const fixtures = createFixtureFactories<Address, KeyedAccountInfo, Test>({
  systemAccount: (value, lamports) => createKeyedSystemAccount(value, lamports),
  programAccount,
  mintAccount: (value, authority, freezeAuthority, supply, decimals, tokenProgram) =>
    createKeyedMintAccount(
      value,
      { decimals, mintAuthority: authority, freezeAuthority, supply },
      new Address(
        tokenProgram === TokenProgram.Token2022
          ? SPL_TOKEN_2022_PROGRAM_ID
          : SPL_TOKEN_PROGRAM_ID,
      ),
    ),
  tokenAccount: (value, mint, owner, amount, tokenProgram) =>
    createKeyedTokenAccount(
      value,
      { amount, mint, owner },
      new Address(
        tokenProgram === TokenProgram.Token2022
          ? SPL_TOKEN_2022_PROGRAM_ID
          : SPL_TOKEN_PROGRAM_ID,
      ),
    ),
});

export const {
  account,
  associatedTokenAccount,
  mint,
  program,
  tokenAccount,
  wallet,
} = fixtures;
