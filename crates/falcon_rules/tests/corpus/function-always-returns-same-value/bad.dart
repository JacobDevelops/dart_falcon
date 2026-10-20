const answer = 6 * 7;
int both(bool flag) { /* expect: function-always-returns-same-value */
  if (flag) { return 2 + 2; } else { return 4; }
}
String trailing(bool flag) { /* expect: function-always-returns-same-value */
  if (flag) return 'same';
  return "same";
}
int lexical(bool flag) { /* expect: function-always-returns-same-value */
  if (flag) return answer;
  return 42;
}
int nested(bool flag) { /* expect: function-always-returns-same-value */
  Object? inner() => null;
  if (flag) { if (flag) return 4; else return 4; }
  return 4;
}
class Example {
  int method(bool flag) { /* expect: function-always-returns-same-value */
    if (flag) return 42;
    return 42;
  }
}

bool constListEquality(bool flag) { /* expect: function-always-returns-same-value */
  if (flag) return const <int>[] == const <int>[];
  return true;
}
class ConstWrapper {
  final List<int> values;
  const ConstWrapper(this.values);
}
bool implicitConstContents(bool flag) { /* expect: function-always-returns-same-value */
  if (flag) return const ConstWrapper([]) == const ConstWrapper([]);
  return true;
}
