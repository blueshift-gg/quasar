import { describe, expect, it } from "vitest";
import { QuasarTestResult, type RawExecutionResult } from "../src/result.js";

interface TestAccount {
  closed: boolean;
  lamports: bigint;
  supply: bigint;
  tokens: bigint;
}

function result(status = { ok: true }): QuasarTestResult<string, TestAccount, RawExecutionResult> {
  const accounts = new Map<string, TestAccount>([
    ["wallet", { closed: false, lamports: 42n, supply: 0n, tokens: 0n }],
    ["mint", { closed: false, lamports: 0n, supply: 55n, tokens: 0n }],
    ["tokens", { closed: false, lamports: 0n, supply: 0n, tokens: 89n }],
    ["closed", { closed: true, lamports: 0n, supply: 0n, tokens: 0n }],
  ]);
  const raw: RawExecutionResult = {
    computeUnits: 99n,
    logs: [],
    status,
    assertCustomError(code) {
      if (code !== 6000) throw new Error(`unexpected code ${code}`);
    },
    assertSuccess() {
      if (!status.ok) throw new Error("execution failed");
    },
  };

  return new QuasarTestResult(raw, {
    account: (address) => accounts.get(address) ?? null,
    isClosed: (account) => account.closed,
    lamports: (account) => account.lamports,
    mintSupply: (account) => account.supply,
    renderAddress: (address) => address,
    tokenBalance: (account) => account.tokens,
  });
}

describe("QuasarTestResult", () => {
  it("chains state and compute assertions", () => {
    const execution = result();

    expect(
      execution
        .succeeds()
        .cuBelow(100)
        .hasLamports("wallet", 42n)
        .hasSupply("mint", 55n)
        .hasTokens("tokens", 89n)
        .isClosed("closed"),
    ).toBe(execution);
  });

  it("accepts typed numeric program errors", () => {
    expect(result({ ok: false }).failsWith(6000)).toBeDefined();
  });

  it("reports assertion mismatches without a test-framework dependency", () => {
    expect(() => result().hasTokens("tokens", 90n)).toThrow(
      "unexpected token balance for tokens: expected 90, got 89",
    );
  });

  it("does not mistake a missing account for a closed account", () => {
    expect(() => result().isClosed("missing")).toThrow(
      "execution result does not contain missing",
    );
  });
});
