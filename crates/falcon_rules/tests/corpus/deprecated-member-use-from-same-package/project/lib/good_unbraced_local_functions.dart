void helper() {}

void bodyScopes(bool condition, int value) {
  helper();
  if (condition)
    @deprecated
    void helper() {}
  helper();
  for (var i = 0; i < value; i++)
    @deprecated
    void helper() {}
  helper();
  while (condition)
    @deprecated
    void helper() {}
  helper();
  do
    @deprecated
    void helper() {}
  while (condition);
  helper();
}
