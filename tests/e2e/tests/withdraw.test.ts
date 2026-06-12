// examples/contracts/withdraw.pact の実行テスト:
// 正常系・異常系(Result)と、requires 違反の構造化エラー(goal 条件 3)。

import { beforeEach, describe, expect, it } from "vitest";
import { PactContractViolation } from "@pact/runtime";

import { withdraw } from "../generated/contracts/withdraw";
import { AccountId, Money } from "../generated/core/money";
import * as Database from "../generated/infra/database";

const alice = AccountId("alice");

beforeEach(() => {
  Database.reset();
});

describe("contracts/withdraw", () => {
  it("残高が足りれば Ok(残額) を返し、残高を更新する", () => {
    Database.seed(alice, Money.of(100));
    const result = withdraw(alice, Money.of(30));
    expect(result.isOk).toBe(true);
    if (result.isOk) {
      expect(result.value).toBe(70);
    }
    expect(Database.balanceOf(alice)).toBe(70);
  });

  it("残高不足なら Err(Overdraft) を返し、残高は変えない", () => {
    Database.seed(alice, Money.of(10));
    const result = withdraw(alice, Money.of(50));
    expect(result.isErr).toBe(true);
    if (result.isErr) {
      expect(result.error.kind).toBe("Overdraft");
      if (result.error.kind === "Overdraft") {
        expect(result.error.fields.limit).toBe(10);
      }
    }
    expect(Database.balanceOf(alice)).toBe(10);
  });

  it("口座が存在しなければ else fail が Err(NotFound) に展開される", () => {
    const result = withdraw(alice, Money.of(5));
    expect(result.isErr).toBe(true);
    if (result.isErr) {
      expect(result.error.kind).toBe("NotFound");
      if (result.error.kind === "NotFound") {
        expect(result.error.values[0]).toBe("alice");
      }
    }
  });

  it("requires 違反は構造化エラー PactContractViolation を投げる", () => {
    Database.seed(alice, Money.of(100));
    let thrown: unknown;
    try {
      withdraw(alice, Money.of(0));
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(PactContractViolation);
    const violation = thrown as PactContractViolation;
    expect(violation.clause).toBe("requires");
    expect(violation.func).toBe("withdraw");
    expect(violation.condition).toBe("amount > Money.zero");
    expect(violation.file).toBe("examples/contracts/withdraw.pact");
    expect(violation.line).toBe(13);
    // 構造化データとして JSON 化できる(診断は JSON が正、散文は派生)。
    expect(violation.toJSON()).toEqual({
      name: "PactContractViolation",
      clause: "requires",
      func: "withdraw",
      condition: "amount > Money.zero",
      file: "examples/contracts/withdraw.pact",
      line: 13,
      col: violation.col,
    });
    // requires 違反時は本体が実行されない(残高不変)。
    expect(Database.balanceOf(alice)).toBe(100);
  });
});
