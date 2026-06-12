module check.effect_transitive

func readFile(path: String) -> String
  uses File.Read
{
  return "data"
}

func loadConfig(path: String) -> String
  uses File.Read
{
  return readFile(path)
}

func boot(path: String) -> String {
  return loadConfig(path)
}
