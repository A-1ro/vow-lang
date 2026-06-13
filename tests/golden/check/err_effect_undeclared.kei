module check.effect_undeclared

func writeRow(id: Int) -> Bool
  uses Database.Write
{
  return true
}

func save(id: Int) -> Bool {
  return writeRow(id)
}

func audit(id: Int) -> Bool
  uses Clock
{
  return writeRow(id)
}
