int next() => 1;
void f(int value, bool condition, List<int> other) {
  final a = [1, 2];
  final b = ['a b', 'ab'];
  final c = [next(), next()];
  final d = [value, value++];
  final e = [value, next(), value];
  final g = [1, ...other, 1];
  final h = [if (condition) 1 else 1];
  final i = [[1], [1]];
  final j = ['$value', '$value'];
}
class Base {
  int value = 0;
  List<int> values() => [value, value];
}
class Changing extends Base {
  int counter = 0;
  @override
  int get value => counter++;
  @override
  set value(int next) {}
}
