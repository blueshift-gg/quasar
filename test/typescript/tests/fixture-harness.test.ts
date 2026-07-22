import { describe, expect, it } from "vitest";
import { AccountRole, address, type Instruction } from "@solana/kit";
import { Address, TransactionInstruction } from "@solana/web3.js";
import {
  Test as KitTest,
  associatedTokenAccount as kitAssociatedTokenAccount,
  mint as kitMint,
  wallet as kitWallet,
  type Fixture as KitFixture,
} from "../src/kit.js";
import {
  Test as Web3Test,
  associatedTokenAccount as web3AssociatedTokenAccount,
  mint as web3Mint,
  wallet as web3Wallet,
  type Fixture as Web3Fixture,
} from "../src/web3.js";

const tokenProgram = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

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
