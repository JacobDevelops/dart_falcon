void checkedThrow(Object value) {
  if (value is Exception) {
    throw value;
  } else {
    throw value; /* expect: only-throw-errors */
  }
}
void castThrow(Object value) {
  value as Exception;
  throw value;
}
void assignedThrow(Object value, Object replacement) {
  if (value is Error) {
    value = replacement;
    throw value; /* expect: only-throw-errors */
  }
}
void shadowThrow(Object value) {
  if (value is! Exception) return;
  {
    Object value = 1;
    throw value; /* expect: only-throw-errors */
  }
  throw value;
}
