import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  getAddressDecoder,
  lamports,
  type Address as KitAddress,
} from "@solana/kit";
import { Address as Web3Address } from "@solana/web3.js";
import { Test as KitTest, wallet as kitWallet } from "../src/kit.js";
import { Test as Web3Test, wallet as web3Wallet } from "../src/web3.js";
import {
  PROGRAM_ADDRESS,
  PROGRAM_ERROR_CODES,
  QuasarVaultClient as KitVaultClient,
  findVaultAddress as findKitVaultAddress,
} from "./fixtures/vault/clients/kit/quasar-vault/client.js";
import {
  PROGRAM_ERROR_CODES as WEB3_PROGRAM_ERROR_CODES,
  QuasarVaultClient as Web3VaultClient,
  findVaultAddress as findWeb3VaultAddress,
} from "./fixtures/vault/clients/web3/quasar-vault/client.js";

const programPath = process.env.QUASAR_PROGRAM_PATH;
if (!programPath) {
  throw new Error("QUASAR_PROGRAM_PATH must point to the compiled Quasar vault");
}

const elfPath = fileURLToPath(new URL(programPath, `file://${process.cwd()}/`));
const userBytes = new Uint8Array(32).fill(1);
const startingLamports = 10_000_000_000n;
const depositAmount = 1_000_000_000n;
const withdrawalAmount = 400_000_000n;

describe("real Quasar program parity", () => {
  it("runs the Rust contract through the Kit adapter", async () => {
    const client = new KitVaultClient();
    const user = getAddressDecoder().decode(userBytes) as KitAddress;
    const vault = await findKitVaultAddress(user);
    using test = await KitTest.load(PROGRAM_ADDRESS, elfPath, {
      computeUnitLimit: 10_000n,
    });
    await test.add(kitWallet({ address: user }));

    const deposit = test
      .send(await client.createDepositInstruction({ user, amount: depositAmount }))
      .succeeds()
      .cuAtMost(1_556n)
      .hasLamports(vault, depositAmount)
      .hasLamports(user, startingLamports - depositAmount);
    expect(deposit.accountChanges.map(change => change.address)).toEqual([
      user,
      vault,
    ]);
    expect(deposit.accountChanges[1]?.before).toBeNull();
    expect(deposit.accountChanges[1]?.wasCreated()).toBe(true);
    expect(deposit.accountChanges[1]?.wasRemoved()).toBe(false);
    expect(deposit.accountChanges[0]?.wasCreated()).toBe(false);
    expect(deposit.accountChanges[0]?.wasRemoved()).toBe(false);

    test.setAccount({
      address: vault,
      data: new Uint8Array(),
      executable: false,
      lamports: lamports(depositAmount),
      programAddress: PROGRAM_ADDRESS,
      space: 0n,
    });
    test
      .simulate(
        await client.createWithdrawInstruction({
          user,
          amount: withdrawalAmount,
        }),
      )
      .succeeds()
      .cuAtMost(392n)
      .hasLamports(vault, depositAmount - withdrawalAmount)
      .hasLamports(
        user,
        startingLamports - depositAmount + withdrawalAmount,
      );
    expect(test.lamports(vault)).toBe(depositAmount);
    expect(test.lamports(user)).toBe(startingLamports - depositAmount);

    const wrongVault = test.freshAddress();
    const rejected = test
      .send(
        await client.createDepositInstructionRaw(
          { user, amount: 1n },
          { vault: wrongVault },
        ),
      )
      .failsWith(PROGRAM_ERROR_CODES.InvalidPda);
    expect(rejected.account(wrongVault)).toBeNull();
    expect(rejected.accountChanges).toEqual([]);
    expect(test.account(wrongVault)).toBeNull();

    test
      .send(
        await client.createWithdrawInstruction({
          user,
          amount: depositAmount + 1n,
        }),
      )
      .fails({ type: "InsufficientFunds" })
      .hasLamports(vault, depositAmount);
    test.warpToTimestamp(42n);

    using limited = await KitTest.load(PROGRAM_ADDRESS, elfPath, {
      computeUnitLimit: 1n,
    });
    await limited.add(kitWallet({ address: user }));
    limited
      .send(await client.createDepositInstruction({ user, amount: 1n }))
      .fails({ type: "Runtime", message: "ProgramFailedToComplete" });
  });

  it("runs the same contract through the Web3.js adapter", async () => {
    const client = new Web3VaultClient();
    const user = new Web3Address(userBytes);
    const vault = await findWeb3VaultAddress(user);
    using test = await Web3Test.load(Web3VaultClient.programId, elfPath, {
      computeUnitLimit: 10_000n,
    });
    await test.add(web3Wallet({ address: user }));

    const deposit = test
      .send(await client.createDepositInstruction({ user, amount: depositAmount }))
      .succeeds()
      .cuAtMost(1_556n)
      .hasLamports(vault, depositAmount)
      .hasLamports(user, startingLamports - depositAmount);
    expect(deposit.accountChanges.map(change => change.address)).toEqual([
      user,
      vault,
    ]);
    expect(deposit.accountChanges[1]?.before).toBeNull();
    expect(deposit.accountChanges[1]?.wasCreated()).toBe(true);
    expect(deposit.accountChanges[0]?.wasCreated()).toBe(false);

    test.setAccount({
      accountId: vault,
      accountInfo: {
        data: new Uint8Array(),
        executable: false,
        lamports: depositAmount,
        owner: Web3VaultClient.programId,
        rentEpoch: 0n,
        space: 0n,
      },
    });
    test
      .simulate(
        await client.createWithdrawInstruction({
          user,
          amount: withdrawalAmount,
        }),
      )
      .succeeds()
      .cuAtMost(392n)
      .hasLamports(vault, depositAmount - withdrawalAmount)
      .hasLamports(
        user,
        startingLamports - depositAmount + withdrawalAmount,
      );
    expect(test.lamports(vault)).toBe(depositAmount);
    expect(test.lamports(user)).toBe(startingLamports - depositAmount);

    const wrongVault = test.freshAddress();
    const rejected = test
      .send(
        await client.createDepositInstructionRaw(
          { user, amount: 1n },
          { vault: wrongVault },
        ),
      )
      .failsWith(WEB3_PROGRAM_ERROR_CODES.InvalidPda);
    expect(rejected.account(wrongVault)).toBeNull();
    expect(rejected.accountChanges).toEqual([]);
    expect(test.account(wrongVault)).toBeNull();

    test
      .send(
        await client.createWithdrawInstruction({
          user,
          amount: depositAmount + 1n,
        }),
      )
      .fails({ type: "InsufficientFunds" })
      .hasLamports(vault, depositAmount);
    test.warpToTimestamp(42n);

    using limited = await Web3Test.load(Web3VaultClient.programId, elfPath, {
      computeUnitLimit: 1n,
    });
    await limited.add(web3Wallet({ address: user }));
    limited
      .send(await client.createDepositInstruction({ user, amount: 1n }))
      .fails({ type: "Runtime", message: "ProgramFailedToComplete" });
  });
});
