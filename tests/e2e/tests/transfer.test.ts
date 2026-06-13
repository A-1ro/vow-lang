// examples/effects/transfer.kei の実行テスト: エフェクトを持つ関数の e2e。

import { beforeEach, describe, expect, it } from "vitest";
import { KeiContractViolation } from "@kei/runtime";

import { transferFunds } from "../generated/effects/transfer";
import { AccountId, Money } from "../generated/core/money";
import * as Audit from "../generated/infra/audit";
import * as Database from "../generated/infra/database";

const alice = AccountId("alice");
const bob = AccountId("bob");

beforeEach(() => {
  Database.reset();
  Audit.reset();
});

describe("effects/transfer", () => {
  it("送金が成功すると Ok(レシート) を返し、残高と監査ログを更新する", () => {
    Database.seed(alice, Money.of(100));
    Database.seed(bob, Money.of(5));
    const result = transferFunds(alice, bob, Money.of(30));
    expect(result.isOk).toBe(true);
    if (result.isOk) {
      expect(result.value).toEqual({ from: "alice", to: "bob", amount: 30 });
    }
    expect(Database.balanceOf(alice)).toBe(70);
    expect(Database.balanceOf(bob)).toBe(35);
    expect(Audit.logEntries()).toEqual([{ from: "alice", to: "bob", amount: 30 }]);
  });

  it("残高不足なら Err(InsufficientFunds) を返す", () => {
    Database.seed(alice, Money.of(100));
    Database.seed(bob, Money.of(5));
    const result = transferFunds(alice, bob, Money.of(200));
    expect(result.isErr).toBe(true);
    if (result.isErr) {
      expect(result.error.kind).toBe("InsufficientFunds");
      if (result.error.kind === "InsufficientFunds") {
        expect(result.error.fields.needed).toBe(200);
        expect(result.error.fields.had).toBe(100);
      }
    }
    expect(Audit.logEntries()).toEqual([]);
  });

  it("送金元が存在しなければ Err(NotFound) を返す", () => {
    Database.seed(bob, Money.of(5));
    const result = transferFunds(alice, bob, Money.of(10));
    expect(result.isErr).toBe(true);
    if (result.isErr) {
      expect(result.error.kind).toBe("NotFound");
    }
  });

  it("requires from != to の違反は構造化エラーを投げる", () => {
    Database.seed(alice, Money.of(100));
    let thrown: unknown;
    try {
      transferFunds(alice, alice, Money.of(10));
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(KeiContractViolation);
    const violation = thrown as KeiContractViolation;
    expect(violation.clause).toBe("requires");
    expect(violation.condition).toBe("from != to");
  });
});
