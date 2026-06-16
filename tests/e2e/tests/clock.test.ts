// examples/effects/clock.kei の実行テスト(extern Time.now() の境界検証 → 実行一致)。

import { describe, expect, it } from "vitest";

import { currentTimestamp } from "../generated/effects/clock";
import { setNow } from "../generated/infra/time";

describe("effects/clock", () => {
  it("currentTimestamp は extern 宣言した Time.now() を呼ぶ", () => {
    setNow(1234);
    expect(currentTimestamp()).toBe(1234);
    setNow(5678);
    expect(currentTimestamp()).toBe(5678);
  });
});
