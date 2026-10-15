// Parentheses make the assertion's parent a grouping expression, not a map entry.
void parenthesizedKey<T>(T? value) {
  final Map<T, int> result = {(value!): 0};
  print(result);
}
void nestedKey<T>(T? value) {
  final Map<T, int> result = {(( /* key */ value! )): 0};
  print(result);
}
void conditionalKey<T>(T? value, bool flag) {
  final Map<T, int> result = {if (flag) /* before */ (value!): 0};
  print(result);
}
void loopKey<T>(T? value) {
  final Map<T, int> result = {for (final number in [0]) ((value!)): number};
  print(result);
}
void nullAwareKey<T>(T? value) {
  final Map<T, int> result = {? /* prefix */ (value!): 0};
  print(result);
}
