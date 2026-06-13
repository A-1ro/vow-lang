module check.contract_impure

func currentBalance(id: Int) -> Int
  uses Database.Read
{
  return 100
}

func withdraw(id: Int, amount: Int) -> Int
  requires currentBalance(id) >= amount
  ensures currentBalance(id) >= 0
{
  return amount
}
