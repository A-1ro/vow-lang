module check.result_option

func findUser(known: Bool) -> Option<Int> {
  if known {
    return Ok(1)
  }
  return None()
}

func loadUser(known: Bool) -> Result<Int, String> {
  if known {
    return Some(2)
  }
  return 3
}

func chain(known: Bool) -> Result<Int, String> {
  let value = loadUser(known) else fail 9
  let direct = known else fail "nope"
  return Ok(value)
}
