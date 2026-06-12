module check.contract_old

func deposit(balance: Int, amount: Int) -> Int
  requires old(balance) >= 0
  requires result >= 0
  ensures result == old(balance) + amount
{
  return balance + amount
}
