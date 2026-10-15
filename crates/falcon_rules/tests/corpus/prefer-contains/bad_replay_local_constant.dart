bool f(List<int> values) {
  const missing = -1;
  const zero = 0;
  if (values.indexOf(1) == missing) return true; /* expect: prefer-contains */
  return values.indexOf(2) >= zero; /* expect: prefer-contains */
}
bool g(List<int> values, int zero) => values.indexOf(1) >= zero;
