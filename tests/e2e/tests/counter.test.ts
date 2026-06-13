// examples/contracts/counter.kei の実行テスト: ensures / old() の実行時検査。

import { describe, expect, it } from "vitest";
import { KeiContractViolation } from "@kei/runtime";

import { increment } from "../generated/contracts/counter";

describe("contracts/counter", () => {
  it("increment は step だけ加算する(ensures result == old(count) + step)", () => {
    expect(increment(1, 2)).toBe(3);
    expect(increment(40, 2)).toBe(42);
  });

  it("requires step > 0 の違反は構造化エラーを投げる", () => {
    let thrown: unknown;
    try {
      increment(1, 0);
    } catch (e) {
      thrown = e;
    }
    expect(thrown).toBeInstanceOf(KeiContractViolation);
    const violation = thrown as KeiContractViolation;
    expect(violation.clause).toBe("requires");
    expect(violation.func).toBe("increment");
    expect(violation.condition).toBe("step > 0");
    expect(violation.file).toBe("examples/contracts/counter.kei");
  });
});
