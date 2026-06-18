// examples/contracts/borrow_direct.kei の実行テスト(#45 / M14 案1):
// 契約を純粋ヘルパーへ退避せず、borrowBook 自身の ensures に外部状態の
// 数量保存(「ちょうど 1 減る」)を直接書く。観測子 Database.availableOf を
// old() でスナップショットし、退出時の値と比較する。

import { beforeEach, describe, expect, it } from "vitest";
import { KeiContractViolation } from "@kei/runtime";

import {
  BookId,
  borrowBook,
  borrowBookOffByTwo,
} from "../generated/contracts/borrow_direct";
import * as Database from "../generated/infra/database";

const dune = BookId("dune");

beforeEach(() => {
  Database.reset();
});

describe("contracts/borrow_direct (effect postcondition / 案1)", () => {
  it("borrowBook は在庫をちょうど 1 減らす — 契約だけ読めば不変条件が分かる", () => {
    Database.seedAvailable(dune, 3);
    expect(borrowBook(dune)).toBe(2);
    // 外部状態(在庫)もちょうど 1 減っている。ensures が実行時に保証した。
    expect(Database.availableOf(dune)).toBe(2);
  });

  it("requires Database.availableOf(book) > 0 の違反は構造化エラー", () => {
    Database.seedAvailable(dune, 0);
    let thrown: unknown;
    try {
      borrowBook(dune);
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(KeiContractViolation);
    expect((thrown as KeiContractViolation).clause).toBe("requires");
  });

  it("反例: 在庫を 2 減らす実装は ensures 違反として実行時に必ず露見する", () => {
    Database.seedAvailable(dune, 5);
    let thrown: unknown;
    try {
      borrowBookOffByTwo(dune);
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(KeiContractViolation);
    const violation = thrown as KeiContractViolation;
    expect(violation.clause).toBe("ensures");
    expect(violation.func).toBe("borrowBookOffByTwo");
    expect(violation.condition).toBe(
      "Database.availableOf(book) == old(Database.availableOf(book)) - 1",
    );
    // 契約が反証した後でも外部状態は実際に 2 減っている(契約は観測するだけ)。
    expect(Database.availableOf(dune)).toBe(3);
  });
});
