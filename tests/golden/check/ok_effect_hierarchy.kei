module check.effect_hierarchy

func writeRow(id: Int) -> Bool
  uses Database.Write
{
  return true
}

func touch(id: Int) -> Bool
  uses IO
{
  return writeRow(id)
}

func mirror(id: Int) -> Bool
  uses Database
{
  return writeRow(id)
}

func chain(id: Int) -> Bool
  uses IO
{
  return touch(id)
}
