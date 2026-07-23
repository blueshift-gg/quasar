export const TokenProgram = {
  Legacy: "legacy",
  Token2022: "token2022",
} as const;

export type TokenProgram = (typeof TokenProgram)[keyof typeof TokenProgram];

export interface Fixture<Output, Host> {
  install(test: Host): Output | Promise<Output>;
}

export interface FixtureHost<Address, Account> {
  freshAddress(): Address;
  setAccount(account: Account): void;
  deriveAta(owner: Address, mint: Address, tokenProgram?: TokenProgram): Promise<Address>;
  loadProgram(programId: Address, elf: Uint8Array, loaderVersion?: number): void;
}

export interface WalletOptions<Address> {
  address?: Address;
  lamports?: bigint;
}

/** A wallet that should hold an associated token account for a mint. */
export interface MintHolder<Address> {
  owner: Address;
  amount?: bigint;
}

export interface MintOptions<Address> {
  address?: Address;
  supply?: bigint;
  decimals?: number;
  tokenProgram?: TokenProgram;
  /**
   * Wallets to fund with an associated token account for this mint, mirroring
   * Rust `Mint::with_holder`. One ATA fixture is installed per holder.
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
    authority: Address,
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
          const address = options.address ?? test.freshAddress();
          test.setAccount(
            factory.systemAccount(
              address,
              options.lamports ?? DEFAULT_WALLET_LAMPORTS,
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

    mint(
      authority: Address,
      options: MintOptions<Address> = {},
    ): Fixture<Address, Host> {
      return {
        async install(test) {
          const address = options.address ?? test.freshAddress();
          const tokenProgram = options.tokenProgram ?? TokenProgram.Legacy;
          test.setAccount(
            factory.mintAccount(
              address,
              authority,
              options.supply ?? 0n,
              options.decimals ?? 6,
              tokenProgram,
            ),
          );
          for (const holder of options.holders ?? []) {
            const ata = await test.deriveAta(holder.owner, address, tokenProgram);
            test.setAccount(
              factory.tokenAccount(
                ata,
                address,
                holder.owner,
                holder.amount ?? 0n,
                tokenProgram,
              ),
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
          const address = options.address ?? test.freshAddress();
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
