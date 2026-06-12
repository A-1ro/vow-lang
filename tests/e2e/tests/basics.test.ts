// examples/basics/ の実行テスト(トランスパイル結果 → 期待出力一致)。

import { describe, expect, it } from "vitest";

import { Point, origin, shift } from "../generated/basics/records";
import { OrderId, OrderStatus, statusCode } from "../generated/basics/enums";
import { firstPositive } from "../generated/basics/options";

describe("basics/records", () => {
  it("origin() は (0, 0) を返す", () => {
    expect(origin()).toEqual({ x: 0, y: 0 });
  });

  it("shift() はフィールドを平行移動した新しい Point を返す", () => {
    const p = Point({ x: 1, y: 2 });
    expect(shift(p, 3, 4)).toEqual({ x: 4, y: 6 });
    expect(p).toEqual({ x: 1, y: 2 });
  });
});

describe("basics/enums", () => {
  it("statusCode() は分岐に応じたコードを返す", () => {
    expect(statusCode(true, false)).toBe(1);
    expect(statusCode(false, true)).toBe(2);
    expect(statusCode(false, false)).toBe(0);
  });

  it("enum は kind 判別の tagged union として動く", () => {
    const draft = OrderStatus.Draft;
    expect(draft.kind).toBe("Draft");

    const submitted = OrderStatus.Submitted(OrderId("order-1"));
    expect(submitted.kind).toBe("Submitted");
    if (submitted.kind === "Submitted") {
      expect(submitted.values[0]).toBe("order-1");
    }

    const rejected = OrderStatus.Rejected({ reason: "out of stock", retryable: true });
    expect(rejected.kind).toBe("Rejected");
    if (rejected.kind === "Rejected") {
      expect(rejected.fields.reason).toBe("out of stock");
      expect(rejected.fields.retryable).toBe(true);
    }
  });
});

describe("basics/options", () => {
  it("最初の正の値を Some で返す", () => {
    const first = firstPositive(3, 9);
    expect(first.isSome).toBe(true);
    if (first.isSome) {
      expect(first.value).toBe(3);
    }

    const second = firstPositive(-1, 9);
    expect(second.isSome).toBe(true);
    if (second.isSome) {
      expect(second.value).toBe(9);
    }
  });

  it("正の値がなければ None を返す", () => {
    const none = firstPositive(-1, 0);
    expect(none.isNone).toBe(true);
  });
});
