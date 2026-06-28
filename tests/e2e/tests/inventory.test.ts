// examples/collections/inventory.kei の実行テスト: List コンビネータ(M9)。
// List<T> は readonly T[] に落ちるので、ホスト側は素の配列を渡せる。

import { describe, expect, it } from "vitest";
import { KeiContractViolation } from "@kei/runtime";

import {
  firstProduct,
  planAllReorders,
  type Product,
  totalStockValue,
} from "../generated/collections/inventory";

const stock: readonly Product[] = [
  { id: "a", quantity: 2, unitPrice: 100, reorderLevel: 5 },
  { id: "b", quantity: 10, unitPrice: 30, reorderLevel: 4 },
  { id: "c", quantity: 0, unitPrice: 50, reorderLevel: 3 },
];

describe("collections/inventory", () => {
  it("totalStockValue は fold で在庫総額を集計する", () => {
    // 2*100 + 10*30 + 0*50 = 500
    expect(totalStockValue(stock)).toBe(500);
    expect(totalStockValue([])).toBe(0);
  });

  it("planAllReorders は filter + map で発注計画を返す(ensures result.length <= products.length)", () => {
    // 発注点を下回るのは a(2<5)と c(0<3)。
    const plans = planAllReorders(stock);
    expect(plans).toEqual([
      { product: "a", orderQuantity: 3 },
      { product: "c", orderQuantity: 3 },
    ]);
    expect(plans.length).toBeLessThanOrEqual(stock.length);
  });

  it("firstProduct は get(0) で先頭を返し、空なら None(範囲外で死なない)", () => {
    const first = firstProduct(stock);
    expect(first.isSome).toBe(true);
    if (first.isSome) {
      expect(first.value.id).toBe("a");
    }
    expect(firstProduct([]).isNone).toBe(true);
  });

  it("requires products.all(p => p.quantity >= 0) の違反は構造化エラー(M25 lambda)", () => {
    const negative: readonly Product[] = [
      { id: "x", quantity: -1, unitPrice: 10, reorderLevel: 2 },
    ];
    let thrown: unknown;
    try {
      totalStockValue(negative);
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(KeiContractViolation);
    const violation = thrown as KeiContractViolation;
    expect(violation.clause).toBe("requires");
    expect(violation.func).toBe("totalStockValue");
    // M25 / #59: 使い捨て述語をその場のラムダで書くことで合意書原則が直読みできる。
    expect(violation.condition).toBe("products.all(p => p.quantity >= 0)");
  });
});
