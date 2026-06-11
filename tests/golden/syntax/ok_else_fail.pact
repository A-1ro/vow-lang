func findUser(id: UserId) -> Result<User, UserError>
  uses Database.Read
{
  let user = Database.fetchUser(id) else fail UserError.NotFound(id)
  return Ok(user)
}
