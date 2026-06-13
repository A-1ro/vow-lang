module check.contract_pure

func nonNegative(value: Int) -> Bool {
  return value >= 0
}

func deposit(balance: Int, amount: Int) -> Int
  requires nonNegative(amount)
  requires amount > 0
  ensures result == old(balance) + amount
  ensures result >= old(balance)
{
  return balance + amount
}
