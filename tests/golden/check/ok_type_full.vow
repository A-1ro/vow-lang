module check.types

type AccountId = String tagged "AccountId"

record Account {
  id: AccountId
  balance: Int
}

enum FetchError {
  Timeout
  NotFound(AccountId)
  Denied { reason: String }
}

func open(id: AccountId, balance: Int) -> Account {
  return Account { id, balance }
}

func describe(account: Account) -> String {
  if account.balance > 0 {
    return "active"
  }
  return "empty"
}

func fetch(id: AccountId, blocked: Bool) -> Result<Account, FetchError> {
  if blocked {
    return Err(FetchError.Denied { reason: "blocked" })
  }
  return Ok(Account { id, balance: 0 })
}

func balanceOf(id: AccountId, known: Bool) -> Option<Int> {
  if known {
    return Some(1)
  }
  return None()
}

func unwrapDemo(id: AccountId, blocked: Bool) -> Result<Int, FetchError> {
  let account = fetch(id, blocked) else fail FetchError.NotFound(id)
  if account.balance == 0 {
    return Err(FetchError.Timeout)
  }
  return Ok(account.balance)
}
