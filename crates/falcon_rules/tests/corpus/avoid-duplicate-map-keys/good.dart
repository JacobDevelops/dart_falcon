int next() => 1;
void f(bool condition, Map<String, int> other) {
  final distinct = {'a': 1, 'b': 2};
  final types = <Object, int>{1: 1, '1': 2};
  final branches = {if (condition) 'a': 1 else 'a': 2};
  final unknown = {'a': 1, ...other};
  final effects = {next(): 1, next(): 2};
  final spaces = {'a b': 1, 'ab': 2};
  final scopes = {'a': {'a': 1}};
  final interpolated = {'$condition': 1, '$condition': 2};
}
