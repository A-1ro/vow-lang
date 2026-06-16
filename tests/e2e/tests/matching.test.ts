// examples/basics/matching.kei の実行テスト(match の網羅分解 → 実行一致)。

import { describe as suite, expect, it } from "vitest";

import { Err, None, Ok, Some } from "@kei/runtime";
import { Light, canGo, describe, isOverdue } from "../generated/basics/matching";

suite("basics/matching", () => {
  it("isOverdue は Option を純粋文脈で分解する(#20)", () => {
    expect(isOverdue(Some(-1))).toEqual(Some(true));
    expect(isOverdue(Some(5))).toEqual(Some(false));
    expect(isOverdue(None())).toEqual(None());
  });

  it("describe は Result を分解して中身を取り出す", () => {
    expect(describe(Ok(3))).toBe("ok");
    expect(describe(Err("boom"))).toBe("boom");
  });

  it("canGo は enum バリアントを網羅分解する", () => {
    expect(canGo(Light.Red)).toBe(false);
    expect(canGo(Light.Yellow)).toBe(false);
    expect(canGo(Light.Green)).toBe(true);
  });
});
