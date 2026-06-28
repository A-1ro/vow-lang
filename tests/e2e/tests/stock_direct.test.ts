// examples/contracts/stock_direct.kei の実行テスト(#56 / M24 在庫ドメイン版):
// borrow_direct と同じ「契約自身が外部状態の数量保存を語る」スタイルを stock で再検証。
// 観測子 Database.quantityOf を old() でスナップショットし、退出時の値と比較する。

import { beforeEach, describe, expect, it } from "vitest";
import { KeiContractViolation } from "@kei/runtime";

import {
  ProductId,
  receiveStock,
  shipStock,
  shipStockForgot,
  shipStockOffByOne,
  shipStockWrongId,
} from "../generated/contracts/stock_direct";
import * as Database from "../generated/infra/database";

const sku = ProductId("sku-001");
const other = ProductId("sku-002");

beforeEach(() => {
  Database.reset();
});

describe("contracts/stock_direct (effect postcondition / 在庫ドメイン版)", () => {
  it("shipStock は在庫をちょうど amount 減らす — 契約だけ読めば不変条件が分かる", () => {
    Database.seedQuantity(sku, 10);
    expect(shipStock(sku, 3)).toBe(7);
    // 外部状態(在庫)もちょうど 3 減っている。ensures が実行時に保証した。
    expect(Database.quantityOf(sku)).toBe(7);
  });

  it("receiveStock は在庫をちょうど amount 増やす", () => {
    Database.seedQuantity(sku, 10);
    expect(receiveStock(sku, 3)).toBe(13);
    expect(Database.quantityOf(sku)).toBe(13);
  });

  it("requires amount > 0 の違反は構造化エラー", () => {
    Database.seedQuantity(sku, 5);
    let thrown: unknown;
    try {
      shipStock(sku, 0);
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(KeiContractViolation);
    expect((thrown as KeiContractViolation).clause).toBe("requires");
  });

  it("反例 A: off-by-one(1 多く減らす)実装は ensures 違反として露見する", () => {
    Database.seedQuantity(sku, 10);
    let thrown: unknown;
    try {
      shipStockOffByOne(sku, 3);
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(KeiContractViolation);
    const violation = thrown as KeiContractViolation;
    expect(violation.clause).toBe("ensures");
    expect(violation.func).toBe("shipStockOffByOne");
    expect(violation.condition).toBe(
      "Database.quantityOf(product) == old(Database.quantityOf(product)) - amount",
    );
    // 契約が反証した後でも外部状態は実際に 1 多く減っている(契約は観測するだけ)。
    expect(Database.quantityOf(sku)).toBe(6);
  });

  it("反例 B: 書き戻し忘れ(在庫が減らない)実装は ensures 違反として露見する", () => {
    Database.seedQuantity(sku, 10);
    let thrown: unknown;
    try {
      shipStockForgot(sku, 3);
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(KeiContractViolation);
    const violation = thrown as KeiContractViolation;
    expect(violation.clause).toBe("ensures");
    expect(violation.func).toBe("shipStockForgot");
    expect(violation.condition).toBe(
      "Database.quantityOf(product) == old(Database.quantityOf(product)) - amount",
    );
    // 書き戻していないので外部状態は元のまま。
    expect(Database.quantityOf(sku)).toBe(10);
  });

  it("反例 C: 別 product id に書く実装は ensures 違反として露見する", () => {
    Database.seedQuantity(sku, 10);
    Database.seedQuantity(other, 10);
    let thrown: unknown;
    try {
      shipStockWrongId(sku, other, 3);
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(KeiContractViolation);
    const violation = thrown as KeiContractViolation;
    expect(violation.clause).toBe("ensures");
    expect(violation.func).toBe("shipStockWrongId");
    expect(violation.condition).toBe(
      "Database.quantityOf(product) == old(Database.quantityOf(product)) - amount",
    );
    // 対象 id(sku)は変化なし。書き込まれたのは別 id(other)の方。
    expect(Database.quantityOf(sku)).toBe(10);
    expect(Database.quantityOf(other)).toBe(7);
  });
});
