module contracts.withdraw

import core.money { Money }
import infra.database as Database

enum WithdrawError {
  NotFound(AccountId)
  Overdraft { limit: Money }
}

func withdraw(account: AccountId, amount: Money) -> Result<Money, WithdrawError>
  uses Database.Write
  requires amount > Money.zero
  ensures result.isOk implies balanceOf(account) == old(balanceOf(account)) - amount
{
  let current = Database.fetchBalance(account) else fail WithdrawError.NotFound(account)
  if current < amount {
    return Err(WithdrawError.Overdraft { limit: current })
  }
  Database.setBalance(account, current - amount)
  return Ok(current - amount)
}
