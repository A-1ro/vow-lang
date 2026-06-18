// e2e スタブ: examples が import する infra.database のインメモリ実装。
// spec §3.3 の「テスト時は DI 的なモジュール差し替えで代替」に相当する。

import { None, Option, Some } from "@kei/runtime";
import { AccountId, Money } from "../core/money";

export type Account = {
  readonly balance: Money;
};

const balances = new Map<string, Money>();

export function reset(): void {
  balances.clear();
  available.clear();
}

export function seed(account: AccountId, balance: Money): void {
  balances.set(account, balance);
}

export function balanceOf(account: AccountId): Money | undefined {
  return balances.get(account);
}

export function fetchBalance(account: AccountId): Option<Money> {
  const balance = balances.get(account);
  return balance === undefined ? None() : Some(balance);
}

export function setBalance(account: AccountId, balance: Money): void {
  balances.set(account, balance);
}

export function fetchAccount(account: AccountId): Option<Account> {
  const balance = balances.get(account);
  return balance === undefined ? None() : Some({ balance });
}

export function debit(account: AccountId, amount: Money): void {
  balances.set(account, (balances.get(account) ?? 0) - amount);
}

export function credit(account: AccountId, amount: Money): void {
  balances.set(account, (balances.get(account) ?? 0) + amount);
}

// 在庫数(examples/contracts/borrow.kei の extern 署名に対応)。
const available = new Map<string, number>();

export function seedAvailable(book: string, count: number): void {
  available.set(book, count);
}

export function fetchAvailable(book: string): Option<number> {
  const count = available.get(book);
  return count === undefined ? None() : Some(count);
}

export function setAvailable(book: string, count: number): void {
  available.set(book, count);
}

// 純粋観測子(extern query)に対応: 在庫数をそのまま返す(未登録は 0)。
// 状態は変えない論理的読み取り。examples/contracts/borrow_direct.kei の契約が呼ぶ。
export function availableOf(book: string): number {
  return available.get(book) ?? 0;
}
