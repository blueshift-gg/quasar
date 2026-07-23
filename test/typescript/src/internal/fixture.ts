export const TokenProgram = {
  Legacy: "legacy",
  Token2022: "token2022",
} as const;

export type TokenProgram = (typeof TokenProgram)[keyof typeof TokenProgram];

export interface Fixture<Output, Host> {
  install(test: Host): Output | Promise<Output>;
}

/**
 * Package-internal key for the deterministic address generator. Not exported to
 * consumers, so `freshAddress` is not part of the public API: tests name actors
 * through `wallet()` and read back the address each fixture returns.
 */
export const FRESH_ADDRESS = Symbol("freshAddress");

export interface FixtureHost<Address, Account> {
  [FRESH_ADDRESS](): Address;
  setAccount(account: Account): void;
  deriveAta(owner: Address, mint: Address, tokenProgram?: TokenProgram): Promise<Address>;
  loadProgram(programId: Address, elf: Uint8Array, loaderVersion?: number): void;
}

export interface WalletOptions<Address> {
  address?: Address;
  /** Exact lamport balance, mirroring Rust `Wallet::fund`. Defaults to
   * `DEFAULT_WALLET_LAMPORTS`. */
  fund?: bigint;
}

/**
 * A wallet and the amount of a mint to seed it with, as an `[owner, amount]`
 * pair. Mirrors one entry of Rust `Mint::with_holder`.
 */
export type MintHolder<Address> = readonly [owner: Address, amount: bigint];

export interface MintOptions<Address> {
  /**
   * Mint authority, mirroring Rust `Mint::with_authority`. Omitted, the mint is
   * fixed-supply (its `mintAuthority` is `COption::None`).
   */
  authority?: Address;
  /**
   * Freeze authority, mirroring Rust `Mint::with_freeze_authority`. Omitted, the
   * mint cannot freeze accounts.
   */
  freezeAuthority?: Address;
  supply?: bigint;
  decimals?: number;
  tokenProgram?: TokenProgram;
  /**
   * Wallets to fund with an associated token account for this mint, mirroring
   * Rust `Mint::with_holder`. One ATA fixture is installed per `[owner, amount]`
   * pair.
   */
  holders?: readonly MintHolder<Address>[];
}

/** A raw, backend-neutral account fixture. Address and owner are required. */
export interface AccountOptions<Address> {
  address: Address;
  owner: Address;
  lamports?: bigint;
  data?: Uint8Array;
}

export interface TokenAccountOptions<Address> {
  address?: Address;
  amount?: bigint;
  tokenProgram?: TokenProgram;
}

export interface AssociatedTokenAccountOptions {
  amount?: bigint;
  tokenProgram?: TokenProgram;
}

/**
 * Default Solana rent-exempt minimum for `dataLen` bytes:
 * `(dataLen + 128) * 3480 * 2`. Matches the runtime's default rent so `write`
 * and the `account` fixture produce rent-exempt accounts without a syscall.
 */
export function rentMinimumBalance(dataLen: number): bigint {
  return BigInt(dataLen + 128) * 3480n * 2n;
}

interface FixtureAccountFactory<Address, Account> {
  systemAccount(address: Address, lamports: bigint): Account;
  programAccount(
    address: Address,
    owner: Address,
    data: Uint8Array,
    lamports: bigint,
  ): Account;
  mintAccount(
    address: Address,
    authority: Address | undefined,
    freezeAuthority: Address | undefined,
    supply: bigint,
    decimals: number,
    tokenProgram: TokenProgram,
  ): Account;
  tokenAccount(
    address: Address,
    mint: Address,
    owner: Address,
    amount: bigint,
    tokenProgram: TokenProgram,
  ): Account;
}

export const DEFAULT_WALLET_LAMPORTS = 10_000_000_000n;

export function createFixtureFactories<
  Address,
  Account,
  Host extends FixtureHost<Address, Account>,
>(factory: FixtureAccountFactory<Address, Account>) {
  return {
    wallet(options: WalletOptions<Address> = {}): Fixture<Address, Host> {
      return {
        install(test) {
          const address = options.address ?? test[FRESH_ADDRESS]();
          test.setAccount(
            factory.systemAccount(
              address,
              options.fund ?? DEFAULT_WALLET_LAMPORTS,
            ),
          );
          return address;
        },
      };
    },

    account(options: AccountOptions<Address>): Fixture<Address, Host> {
      return {
        install(test) {
          const data = options.data ?? new Uint8Array();
          test.setAccount(
            factory.programAccount(
              options.address,
              options.owner,
              data,
              options.lamports ?? rentMinimumBalance(data.length),
            ),
          );
          return options.address;
        },
      };
    },

    mint(options: MintOptions<Address> = {}): Fixture<Address, Host> {
      return {
        async install(test) {
          const address = test[FRESH_ADDRESS]();
          const tokenProgram = options.tokenProgram ?? TokenProgram.Legacy;
          test.setAccount(
            factory.mintAccount(
              address,
              options.authority,
              options.freezeAuthority,
              options.supply ?? 0n,
              options.decimals ?? 6,
              tokenProgram,
            ),
          );
          for (const [owner, amount] of options.holders ?? []) {
            const ata = await test.deriveAta(owner, address, tokenProgram);
            test.setAccount(
              factory.tokenAccount(ata, address, owner, amount, tokenProgram),
            );
          }
          return address;
        },
      };
    },

    tokenAccount(
      mint: Address,
      owner: Address,
      options: TokenAccountOptions<Address> = {},
    ): Fixture<Address, Host> {
      return {
        install(test) {
          const address = options.address ?? test[FRESH_ADDRESS]();
          test.setAccount(
            factory.tokenAccount(
              address,
              mint,
              owner,
              options.amount ?? 0n,
              options.tokenProgram ?? TokenProgram.Legacy,
            ),
          );
          return address;
        },
      };
    },

    associatedTokenAccount(
      mint: Address,
      owner: Address,
      options: AssociatedTokenAccountOptions = {},
    ): Fixture<Address, Host> {
      return {
        async install(test) {
          const tokenProgram = options.tokenProgram ?? TokenProgram.Legacy;
          const address = await test.deriveAta(owner, mint, tokenProgram);
          test.setAccount(
            factory.tokenAccount(
              address,
              mint,
              owner,
              options.amount ?? 0n,
              tokenProgram,
            ),
          );
          return address;
        },
      };
    },

    program(
      programId: Address,
      elf: Uint8Array,
      loaderVersion?: number,
    ): Fixture<Address, Host> {
      return {
        install(test) {
          test.loadProgram(programId, elf, loaderVersion);
          return programId;
        },
      };
    },
  };
}
