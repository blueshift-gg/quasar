import { describe, expect, it } from "vitest";
import { QuasarTestResult, type RawExecutionResult } from "../src/result.js";

interface TestAccount {
  closed: boolean;
  lamports: bigint;
  supply: bigint;
  tokens: bigint;
}

const EXPECTED_ERROR_CODE = 6000;

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
      // Mirror the real adapters: a custom-error assertion must reject a
      // successful execution, not only a wrong code.
      if (status.ok) throw new Error("execution succeeded");
      if (code !== EXPECTED_ERROR_CODE) throw new Error(`unexpected code ${code}`);
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

  it("failsWith accepts the exact custom error and returns the chain", () => {
    const execution = result({ ok: false });
    expect(execution.failsWith(EXPECTED_ERROR_CODE)).toBe(execution);
  });

  it("failsWith rejects a wrong error code", () => {
    expect(() => result({ ok: false }).failsWith(6001)).toThrow("unexpected code 6001");
  });

  it("failsWith rejects a successful execution", () => {
    expect(() => result({ ok: true }).failsWith(EXPECTED_ERROR_CODE)).toThrow(
      "execution succeeded",
    );
  });

  it("succeeds rejects a failed execution", () => {
    expect(() => result({ ok: false }).succeeds()).toThrow("execution failed");
  });

  it("cuBelow rejects consumption at or above the limit", () => {
    expect(() => result().cuBelow(99)).toThrow(
      "expected fewer than 99 compute units, consumed 99",
    );
    expect(() => result().cuBelow(50n)).toThrow(
      "expected fewer than 50 compute units, consumed 99",
    );
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

  it("state assertions reject addresses missing from the result", () => {
    expect(() => result().hasLamports("missing", 1n)).toThrow(
      "execution result does not contain missing",
    );
  });

  it("isClosed rejects a live account", () => {
    expect(() => result().isClosed("wallet")).toThrow("expected wallet to be closed");
  });
});
