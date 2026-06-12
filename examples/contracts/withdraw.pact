module contracts.withdraw

import core.money { AccountId, Money }
import infra.database as Database

enum WithdrawError {
  NotFound(AccountId)
  Overdraft { limit: Money }
}

func withdraw(account: AccountId, amount: Money) -> Result<Money, WithdrawError>
  uses Database.Read, Database.Write
  requires amount > Money.zero
  ensures result.isOk implies amount > Money.zero
{
  let current = Database.fetchBalance(account) else fail WithdrawError.NotFound(account)
  if current < amount {
    return Err(WithdrawError.Overdraft { limit: current })
  }
  Database.setBalance(account, current - amount)
  return Ok(current - amount)
}
