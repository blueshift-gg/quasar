import {
  AccountState,
  getMintEncoder,
  getMintSize,
  getTokenEncoder,
  getTokenSize,
} from "@solana-program/token";
import type { Address as SplAddress } from "@solana/kit";
import { rentMinimumBalance } from "./fixture.js";

/**
 * SPL and native program IDs. Reimplemented locally so the harness owns its own
 * SPL layouts and does not depend on a private native backend.
 */
export const SPL_TOKEN_PROGRAM_ID = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
export const SPL_TOKEN_2022_PROGRAM_ID =
  "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
export const SPL_ASSOCIATED_TOKEN_PROGRAM_ID =
  "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
export const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";

const mintEncoder = getMintEncoder();
const tokenEncoder = getTokenEncoder();

/** Backend-neutral account bytes: owner program, encoded body, rent-exempt lamports. */
export interface RawAccount {
  readonly owner: string;
  readonly data: Uint8Array;
  readonly lamports: bigint;
}

/** A system-owned account holding `lamports` and no data. */
export function systemAccountData(lamports: bigint): RawAccount {
  return { owner: SYSTEM_PROGRAM_ID, data: new Uint8Array(0), lamports };
}

export interface MintFields {
  readonly mintAuthority?: string;
  readonly freezeAuthority?: string;
  readonly supply?: bigint;
  readonly decimals?: number;
}

/** A pre-initialized SPL mint, owned by `tokenProgram`. */
export function mintAccountData(
  fields: MintFields,
  tokenProgram: string,
): RawAccount {
  const data = new Uint8Array(
    mintEncoder.encode({
      mintAuthority: (fields.mintAuthority ?? null) as SplAddress | null,
      supply: fields.supply ?? 0n,
      decimals: fields.decimals ?? 9,
      isInitialized: true,
      freezeAuthority: (fields.freezeAuthority ?? null) as SplAddress | null,
    }),
  );
  return { owner: tokenProgram, data, lamports: rentMinimumBalance(getMintSize()) };
}

export interface TokenFields {
  readonly mint: string;
  readonly owner: string;
  readonly amount: bigint;
}

/** A pre-initialized SPL token account, owned by `tokenProgram`. */
export function tokenAccountData(
  fields: TokenFields,
  tokenProgram: string,
): RawAccount {
  const data = new Uint8Array(
    tokenEncoder.encode({
      mint: fields.mint as SplAddress,
      owner: fields.owner as SplAddress,
      amount: fields.amount,
      delegate: null,
      state: AccountState.Initialized,
      isNative: null,
      delegatedAmount: 0n,
      closeAuthority: null,
    }),
  );
  return {
    owner: tokenProgram,
    data,
    lamports: rentMinimumBalance(getTokenSize()),
  };
}
