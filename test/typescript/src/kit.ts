import {
  createKeyedMintAccount,
  createKeyedSystemAccount,
  createKeyedTokenAccount,
  QuasarSvm,
  SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
  SPL_TOKEN_2022_PROGRAM_ID,
  SPL_TOKEN_PROGRAM_ID,
} from "@blueshift-gg/quasar-svm/kit";
import { getMintDecoder, getTokenDecoder } from "@solana-program/token";
import {
  address,
  AccountRole,
  getAddressDecoder,
  getAddressEncoder,
  getProgramDerivedAddress,
  isSignerRole,
  isWritableRole,
  lamports,
  type Account,
  type Address,
  type Instruction,
} from "@solana/kit";
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

type WorldAccount = Account<Uint8Array>;
export type Fixture<Output> = SharedFixture<Output, Test>;
export type Outcome = SharedOutcome<Address, WorldAccount>;
export type AccountChange = SharedAccountChange<Address, WorldAccount>;
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
): { address: Address; role: AccountRole }[] {
  return addresses.map(address => ({
    address,
    role: AccountRole.READONLY_SIGNER,
  }));
}

/** Value-equality for addresses, independent of the backend representation. */
export function addressesEqual(left: Address, right: Address): boolean {
  return left === right;
}

const addressEncoder = getAddressEncoder();
const systemProgram = address("11111111111111111111111111111111");

function bytesEqual(left: Uint8Array, right: Uint8Array): boolean {
  return (
    left.length === right.length &&
    left.every((byte, index) => byte === right[index])
  );
}

function encoded(value: Address): Uint8Array {
  return new Uint8Array(addressEncoder.encode(value));
}

function programAccount(
  value: Address,
  owner: Address,
  data: Uint8Array,
  lamps: bigint,
): WorldAccount {
  return {
    address: value,
    programAddress: owner,
    lamports: lamports(lamps),
    data,
    executable: false,
    space: BigInt(data.length),
  };
}

const adapter: HarnessAdapter<
  Address,
  WorldAccount,
  Instruction,
  Outcome
> = {
  addressKey: value => value,
  freshAddress: bytes => getAddressDecoder().decode(bytes) as Address,
  accountAddress: account => account.address,
  accountData: account => account.data,
  accountOwner: account => account.programAddress,
  accountLamports: account => BigInt(account.lamports),
  instructionAccounts: instruction =>
    (instruction.accounts ?? []).map(meta => ({
      address: meta.address,
      writable: isWritableRole(meta.role),
      signer: isSignerRole(meta.role),
    })),
  emptyAccount: value => createKeyedSystemAccount(value, 0n),
  fundedAccount: value =>
    createKeyedSystemAccount(value, DEFAULT_WALLET_LAMPORTS),
  programAccount,
  tokenAmount: account => BigInt(getTokenDecoder().decode(account.data).amount),
  mintSupply: account => BigInt(getMintDecoder().decode(account.data).supply),
  accountsEqual: (left, right) =>
    left === null
      ? right === null
      : right !== null &&
        left.address === right.address &&
        left.programAddress === right.programAddress &&
        BigInt(left.lamports) === BigInt(right.lamports) &&
        left.executable === right.executable &&
        bytesEqual(left.data, right.data),
  async deriveAta(owner, mint, tokenProgram) {
    const tokenProgramId = address(
      tokenProgram === TokenProgram.Token2022
        ? SPL_TOKEN_2022_PROGRAM_ID
        : SPL_TOKEN_PROGRAM_ID,
    );
    return (await getProgramDerivedAddress({
      programAddress: address(SPL_ASSOCIATED_TOKEN_PROGRAM_ID),
      seeds: [encoded(owner), encoded(tokenProgramId), encoded(mint)],
    }))[0];
  },
  deriveProgramAddress: (seeds, programAddress) =>
    getProgramDerivedAddress({ programAddress, seeds: [...seeds] }),
  outcome: (raw, accounts, changes) =>
    new SharedOutcome(raw, accounts, adapter, changes),
  isClosed: account =>
    BigInt(account.lamports) === 0n &&
    account.data.length === 0 &&
    account.programAddress === systemProgram,
  lamports: account => BigInt(account.lamports),
  renderAddress: value => value,
};

/** An isolated fixture-first test world using Kit address and account types. */
export class Test extends TestCore<
  Address,
  WorldAccount,
  Instruction,
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

const fixtures = createFixtureFactories<Address, WorldAccount, Test>({
  systemAccount: (value, lamps) => createKeyedSystemAccount(value, lamps),
  programAccount,
  mintAccount: (value, authority, freezeAuthority, supply, decimals, tokenProgram) =>
    createKeyedMintAccount(
      value,
      { decimals, mintAuthority: authority, freezeAuthority, supply },
      address(
        tokenProgram === TokenProgram.Token2022
          ? SPL_TOKEN_2022_PROGRAM_ID
          : SPL_TOKEN_PROGRAM_ID,
      ),
    ),
  tokenAccount: (value, mint, owner, amount, tokenProgram) =>
    createKeyedTokenAccount(
      value,
      { amount, mint, owner },
      address(
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
