module effects.transfer

import core.money { AccountId, Money }
import infra.audit as Audit
import infra.database as Database

record TransferReceipt {
  from: AccountId
  to: AccountId
  amount: Money
}

enum TransferError {
  NotFound(AccountId)
  InsufficientFunds { needed: Money, had: Money }
}

func transferFunds(from: AccountId, to: AccountId, amount: Money) -> Result<TransferReceipt, TransferError>
  uses Database.Read, Database.Write, Audit.Log
  requires amount > Money.zero
  requires from != to
{
  let sender = Database.fetchAccount(from) else fail TransferError.NotFound(from)
  if sender.balance < amount {
    return Err(TransferError.InsufficientFunds { needed: amount, had: sender.balance })
  }
  Database.debit(from, amount)
  Database.credit(to, amount)
  let receipt = TransferReceipt { from, to, amount }
  Audit.Log.record(receipt)
  return Ok(receipt)
}
