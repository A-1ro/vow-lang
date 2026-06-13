// app/greet の実行テスト。record が readonly object + 同名ファクトリとして動くこと。

import { describe, expect, it } from "vitest";

import { Greeting, bump } from "../dist/app/greet";

describe("app/greet", () => {
  it("bump は count を 1 増やした新しい Greeting を返す(元は不変)", () => {
    const g = Greeting({ count: 1 });
    expect(bump(g)).toEqual({ count: 2 });
    expect(g).toEqual({ count: 1 });
  });
});
