void compare(List<int> values, dynamic unknown) {
  const a = [1];
  const b = [1];
  print(a == b);
  print(const <int>{1} == const <int>{1});
  print(const <String, int>{'x': 1} == const <String, int>{'x': 1});
  print(values == null);
  print(values == unknown);
  print(values == 1);
  print(1 == 2);
  print(identical(values, values));
}

class ValueList implements List<int> {
  @override
  bool operator ==(Object other) => other is ValueList;
  @override
  int get hashCode => 0;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
void custom(ValueList first, ValueList second) { print(first == second); }
void unrelated(List<int> integers, List<String> strings, Set<int> unique) {
  print(integers == strings);
  print(integers == unique);
}
