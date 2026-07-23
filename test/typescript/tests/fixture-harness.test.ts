import { describe, expect, it } from "vitest";
import { AccountRole, address, type Instruction } from "@solana/kit";
import { Address, TransactionInstruction } from "@solana/web3.js";
import { getTokenDecoder } from "@solana-program/token";
import {
  Test as KitTest,
  account as kitAccount,
  addressesEqual as kitAddressesEqual,
  associatedTokenAccount as kitAssociatedTokenAccount,
  coSigners as kitCoSigners,
  mint as kitMint,
  wallet as kitWallet,
  type AccountCodec as KitAccountCodec,
  type Fixture as KitFixture,
} from "../src/kit.js";
import {
  Test as Web3Test,
  account as web3Account,
  addressesEqual as web3AddressesEqual,
  associatedTokenAccount as web3AssociatedTokenAccount,
  coSigners as web3CoSigners,
  mint as web3Mint,
  wallet as web3Wallet,
  type AccountCodec as Web3AccountCodec,
  type Fixture as Web3Fixture,
} from "../src/web3.js";

const tokenProgram = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

const COUNTER_DISCRIMINATOR = new Uint8Array([7]);

/** A hand-built codec exercising discriminator/owner/size framing. */
function counterCodec<A>(owner: A) {
  return {
    owner,
    discriminator: COUNTER_DISCRIMINATOR,
    size: COUNTER_DISCRIMINATOR.length + 8,
    decode(bytes: Uint8Array) {
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      return { count: view.getBigUint64(0, true) };
    },
    encode(value: { count: bigint }) {
      const body = new Uint8Array(8);
      new DataView(body.buffer).setBigUint64(0, value.count, true);
      return body;
    },
  };
}

function transferData(amount: bigint): Uint8Array {
  const data = new Uint8Array(9);
  data[0] = 3;
  new DataView(data.buffer).setBigUint64(1, amount, true);
  return data;
}

describe("fixture-first test harness", () => {
  it("provides the Kit fixture and outcome path", async () => {
    using test = new KitTest();
    const [authority, recipient] = await test.add([
      kitWallet(),
      kitWallet(),
    ] as const);
    const mint = await test.add(kitMint(authority, { supply: 10_000n }));
    const alice = await test.add(
      kitAssociatedTokenAccount(mint, authority, { amount: 5_000n }),
    );
    const bob = await test.add(kitAssociatedTokenAccount(mint, recipient));

    const transfer: Instruction = {
      programAddress: address(tokenProgram),
      accounts: [
        { address: alice, role: AccountRole.WRITABLE },
        { address: bob, role: AccountRole.WRITABLE },
        { address: authority, role: AccountRole.READONLY_SIGNER },
      ],
      data: transferData(1_000n),
    };

    const outcome = test
      .send(transfer)
      .succeeds()
      .hasTokens(alice, 4_000n)
      .hasTokens(bob, 1_000n)
      .cuAtMost(20_000n);
    expect(outcome.accountChanges.map(change => change.address)).toEqual([
      alice,
      bob,
    ]);
    expect(test.supply(mint)).toBe(10_000n);

    test
      .send({ ...transfer, data: transferData(10_000n) })
      .failsWith(1)
      .hasTokens(alice, 4_000n)
      .hasTokens(bob, 1_000n);

    test.simulate(transfer).succeeds().hasTokens(bob, 2_000n);
    expect(test.tokens(bob)).toBe(1_000n);

    const protocol: KitFixture<readonly [typeof authority, typeof mint]> = {
      install: () => [authority, mint] as const,
    };
    expect(await test.add(protocol)).toEqual([authority, mint]);
  });

  it("provides the same fixture and outcome path for Web3.js", async () => {
    using test = new Web3Test();
    const [authority, recipient] = await test.add([
      web3Wallet(),
      web3Wallet(),
    ] as const);
    const mint = await test.add(web3Mint(authority, { supply: 10_000n }));
    const alice = await test.add(
      web3AssociatedTokenAccount(mint, authority, { amount: 5_000n }),
    );
    const bob = await test.add(web3AssociatedTokenAccount(mint, recipient));

    const transfer = new TransactionInstruction({
      programId: new Address(tokenProgram),
      keys: [
        { pubkey: alice, isSigner: false, isWritable: true },
        { pubkey: bob, isSigner: false, isWritable: true },
        { pubkey: authority, isSigner: true, isWritable: false },
      ],
      data: transferData(1_000n),
    });

    const outcome = test
      .send(transfer)
      .succeeds()
      .hasTokens(alice, 4_000n)
      .hasTokens(bob, 1_000n)
      .cuAtMost(20_000n);
    expect(
      outcome.accountChanges.map(change => change.address.toBase58()),
    ).toEqual([alice.toBase58(), bob.toBase58()]);
    expect(test.supply(mint)).toBe(10_000n);

    test
      .send(
        new TransactionInstruction({
          programId: new Address(tokenProgram),
          keys: transfer.keys,
          data: transferData(10_000n),
        }),
      )
      .failsWith(1)
      .hasTokens(alice, 4_000n)
      .hasTokens(bob, 1_000n);

    test.simulate(transfer).succeeds().hasTokens(bob, 2_000n);
    expect(test.tokens(bob)).toBe(1_000n);

    const protocol: Web3Fixture<readonly [Address, Address]> = {
      install: () => [authority, mint] as const,
    };
    expect((await test.add(protocol)).map(value => value.toBase58())).toEqual([
      authority.toBase58(),
      mint.toBase58(),
    ]);
  });

  it("uses the same deterministic fixture addresses in both adapters", async () => {
    using kit = new KitTest();
    using web3 = new Web3Test();
    const kitAddress = await kit.add(kitWallet());
    const web3Address = await web3.add(web3Wallet());
    expect(kitAddress).toBe(web3Address.toBase58());
  });

  it("validates stable runtime limits before entering either backend", () => {
    using zeroKit = new KitTest(undefined, undefined, { computeUnitLimit: 0n });
    using zeroWeb3 = new Web3Test(undefined, undefined, {
      computeUnitLimit: 0n,
    });
    expect(
      () => new KitTest(undefined, undefined, { computeUnitLimit: -1n }),
    ).toThrow("computeUnitLimit must fit a u64");
    expect(
      () =>
        new KitTest(undefined, undefined, {
          computeUnitLimit: 0x1_0000_0000_0000_0000n,
        }),
    ).toThrow("computeUnitLimit must fit a u64");
    expect(() => zeroKit.warpToTimestamp(-0x8000_0000_0000_0001n)).toThrow(
      "timestamp must fit an i64",
    );
    expect(() => zeroWeb3.warpToTimestamp(0x8000_0000_0000_0000n)).toThrow(
      "timestamp must fit an i64",
    );
  });
});

describe("typed account ergonomics", () => {
  it("reads and writes typed accounts and installs raw accounts (Kit)", async () => {
    using test = new KitTest();
    const owner = test.freshAddress();
    const codec = counterCodec(owner);
    const counter = test.write(codec, test.freshAddress(), { count: 42n });

    expect(test.read(codec, counter)).toEqual({ count: 42n });
    expect(kitAddressesEqual(test.account(counter)!.programAddress, owner)).toBe(
      true,
    );
    expect(test.lamports(counter)).toBe(BigInt(9 + 128) * 3480n * 2n);

    expect(() => test.read(counterCodec(test.freshAddress()), counter)).toThrow(
      /owned by/,
    );
    expect(() => test.read(codec, test.freshAddress())).toThrow(/no account/);

    const wrongDisc = await test.add(
      kitAccount({
        address: test.freshAddress(),
        owner,
        data: new Uint8Array([9, 0, 0, 0, 0, 0, 0, 0, 0]),
      }),
    );
    expect(() => test.read(codec, wrongDisc)).toThrow(/discriminator/);

    const tooSmall = await test.add(
      kitAccount({ address: test.freshAddress(), owner, data: new Uint8Array([7, 0, 0]) }),
    );
    expect(() => test.read(codec, tooSmall)).toThrow(/at least/);
  });

  it("reads and writes typed accounts and installs raw accounts (Web3.js)", async () => {
    using test = new Web3Test();
    const owner = test.freshAddress();
    const codec = counterCodec(owner);
    const counter = test.write(codec, test.freshAddress(), { count: 42n });

    expect(test.read(codec, counter)).toEqual({ count: 42n });
    expect(
      web3AddressesEqual(test.account(counter)!.accountInfo.owner, owner),
    ).toBe(true);
    expect(test.lamports(counter)).toBe(BigInt(9 + 128) * 3480n * 2n);

    expect(() => test.read(counterCodec(test.freshAddress()), counter)).toThrow(
      /owned by/,
    );
    expect(() => test.read(codec, test.freshAddress())).toThrow(/no account/);

    const wrongDisc = await test.add(
      web3Account({
        address: test.freshAddress(),
        owner,
        data: new Uint8Array([9, 0, 0, 0, 0, 0, 0, 0, 0]),
      }),
    );
    expect(() => test.read(codec, wrongDisc)).toThrow(/discriminator/);
  });

  it("asserts decoded account state via hasState and read (Kit)", async () => {
    using test = new KitTest();
    const [authority, recipient] = await test.add([
      kitWallet(),
      kitWallet(),
    ] as const);
    const mint = await test.add(
      kitMint(authority, {
        supply: 10_000n,
        holders: [{ owner: authority, amount: 5_000n }],
      }),
    );
    const alice = await test.deriveAta(authority, mint);
    const bob = await test.add(kitAssociatedTokenAccount(mint, recipient));

    const transfer: Instruction = {
      programAddress: address(tokenProgram),
      accounts: [
        { address: alice, role: AccountRole.WRITABLE },
        { address: bob, role: AccountRole.WRITABLE },
        { address: authority, role: AccountRole.READONLY_SIGNER },
      ],
      data: transferData(1_000n),
    };

    const tokenCodec = {
      owner: address(tokenProgram),
      decode: (bytes: Uint8Array) => getTokenDecoder().decode(bytes),
    } satisfies KitAccountCodec<{ amount: bigint }>;

    test
      .send(transfer)
      .succeeds()
      .hasState(tokenCodec, alice, state =>
        expect(BigInt(state.amount)).toBe(4_000n),
      )
      .hasState(tokenCodec, bob, state =>
        expect(BigInt(state.amount)).toBe(1_000n),
      );

    expect(BigInt(test.read(tokenCodec, alice).amount)).toBe(4_000n);
    expect(() =>
      test.read(
        {
          owner: test.freshAddress(),
          decode: (bytes: Uint8Array) => getTokenDecoder().decode(bytes),
        },
        alice,
      ),
    ).toThrow(/owned by/);
  });

  it("asserts decoded account state via hasState and read (Web3.js)", async () => {
    using test = new Web3Test();
    const [authority, recipient] = await test.add([
      web3Wallet(),
      web3Wallet(),
    ] as const);
    const mint = await test.add(
      web3Mint(authority, {
        supply: 10_000n,
        holders: [{ owner: authority, amount: 5_000n }],
      }),
    );
    const alice = await test.deriveAta(authority, mint);
    const bob = await test.add(web3AssociatedTokenAccount(mint, recipient));

    const transfer = new TransactionInstruction({
      programId: new Address(tokenProgram),
      keys: [
        { pubkey: alice, isSigner: false, isWritable: true },
        { pubkey: bob, isSigner: false, isWritable: true },
        { pubkey: authority, isSigner: true, isWritable: false },
      ],
      data: transferData(1_000n),
    });

    const tokenCodec = {
      owner: new Address(tokenProgram),
      decode: (bytes: Uint8Array) => getTokenDecoder().decode(bytes),
    } satisfies Web3AccountCodec<{ amount: bigint }>;

    test
      .send(transfer)
      .succeeds()
      .hasState(tokenCodec, alice, state =>
        expect(BigInt(state.amount)).toBe(4_000n),
      )
      .hasState(tokenCodec, bob, state =>
        expect(BigInt(state.amount)).toBe(1_000n),
      );

    expect(BigInt(test.read(tokenCodec, alice).amount)).toBe(4_000n);
  });

  it("funds mint holders with associated token accounts", async () => {
    using kit = new KitTest();
    const [kitAuthority, kitAlice, kitBob] = await kit.add([
      kitWallet(),
      kitWallet(),
      kitWallet(),
    ] as const);
    const kitMintAddress = await kit.add(
      kitMint(kitAuthority, {
        supply: 9_000n,
        holders: [{ owner: kitAlice, amount: 5_000n }, { owner: kitBob }],
      }),
    );
    expect(kit.tokens(await kit.deriveAta(kitAlice, kitMintAddress))).toBe(
      5_000n,
    );
    expect(kit.tokens(await kit.deriveAta(kitBob, kitMintAddress))).toBe(0n);
    expect(kit.supply(kitMintAddress)).toBe(9_000n);

    using web3 = new Web3Test();
    const [web3Authority, web3Alice] = await web3.add([
      web3Wallet(),
      web3Wallet(),
    ] as const);
    const web3MintAddress = await web3.add(
      web3Mint(web3Authority, { holders: [{ owner: web3Alice, amount: 7_000n }] }),
    );
    expect(web3.tokens(await web3.deriveAta(web3Alice, web3MintAddress))).toBe(
      7_000n,
    );
  });

  it("builds co-signer metas and auto-registers missing signers (Kit)", async () => {
    using test = new KitTest();
    const [authority, recipient] = await test.add([
      kitWallet(),
      kitWallet(),
    ] as const);
    const mint = await test.add(
      kitMint(authority, {
        supply: 2_000n,
        holders: [{ owner: authority, amount: 2_000n }],
      }),
    );
    const alice = await test.deriveAta(authority, mint);
    const bob = await test.add(kitAssociatedTokenAccount(mint, recipient));

    const extra = test.freshAddress();
    const cosigners = kitCoSigners([extra]);
    expect(cosigners).toEqual([
      { address: extra, role: AccountRole.READONLY_SIGNER },
    ]);
    expect(test.account(extra)).toBeNull();

    const transfer: Instruction = {
      programAddress: address(tokenProgram),
      accounts: [
        { address: alice, role: AccountRole.WRITABLE },
        { address: bob, role: AccountRole.WRITABLE },
        { address: authority, role: AccountRole.READONLY_SIGNER },
        ...cosigners,
      ],
      data: transferData(500n),
    };

    test.send(transfer).succeeds().hasTokens(bob, 500n);
  });

  it("builds co-signer metas and auto-registers missing signers (Web3.js)", async () => {
    using test = new Web3Test();
    const [authority, recipient] = await test.add([
      web3Wallet(),
      web3Wallet(),
    ] as const);
    const mint = await test.add(
      web3Mint(authority, {
        supply: 2_000n,
        holders: [{ owner: authority, amount: 2_000n }],
      }),
    );
    const alice = await test.deriveAta(authority, mint);
    const bob = await test.add(web3AssociatedTokenAccount(mint, recipient));

    const extra = test.freshAddress();
    const cosigners = web3CoSigners([extra]);
    expect(cosigners).toEqual([
      { pubkey: extra, isSigner: true, isWritable: false },
    ]);

    const transfer = new TransactionInstruction({
      programId: new Address(tokenProgram),
      keys: [
        { pubkey: alice, isSigner: false, isWritable: true },
        { pubkey: bob, isSigner: false, isWritable: true },
        { pubkey: authority, isSigner: true, isWritable: false },
        ...cosigners,
      ],
      data: transferData(500n),
    });

    test.send(transfer).succeeds().hasTokens(bob, 500n);
  });
});
