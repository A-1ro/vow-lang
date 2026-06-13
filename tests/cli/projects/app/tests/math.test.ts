// app/math の実行テスト。`kei test` が dev ビルド(契約 on)した dist を呼ぶ。
//
// requires 違反の扱いを環境変数で切り替える:
// - 既定: KeiContractViolation が投げられることを assert(契約が焼き込まれている証拠)。
// - KEI_EXPECT_VIOLATION=uncaught: 違反を捕捉せず素通しさせ、テスト失敗 →
//   `npm test` 非ゼロ終了 → `kei test` 非ゼロ終了、を CLI 統合テストから観測する。

import { describe, expect, it } from "vitest";
import { KeiContractViolation } from "@kei/runtime";

import { addPositive } from "../dist/app/math";

describe("app/math", () => {
  it("正の整数を足す", () => {
    expect(addPositive(2, 3)).toBe(5);
  });

  it("requires を破ると契約違反になる(dev ビルドは契約 on)", () => {
    if (process.env.KEI_EXPECT_VIOLATION === "uncaught") {
      // requires a > 0 を破る。dev ビルドでは KeiContractViolation が投げられ、
      // ここで捕捉しないので vitest がこのテストを失敗として非ゼロ終了する。
      addPositive(0, 3);
    } else {
      expect(() => addPositive(0, 3)).toThrow(KeiContractViolation);
    }
  });
});
