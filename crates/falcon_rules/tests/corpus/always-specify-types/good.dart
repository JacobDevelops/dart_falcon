int count = 0;
final String name = 'x';
const int limit = 3;
List<int> typed = <int>[];

class Box<T> {
  final T value;
  Box(this.value);
}

class Plain {
  final int id;
  Plain(this.id);
}

class Holder {
  final List<int> items = <int>[];
  Map<String, int> lookup = <String, int>{};
  Box<int> box = Box<int>(1);
  Plain plain = Plain(1);
}

void f(Object o, int y, void Function(int) cb) {
  int local = 1;
  final List<int> list = <int>[];
  final Map<String, int> map = <String, int>{};
  for (int i = 0; i < 2; i++) {}
  for (final int e in list) {}
  final Box<int> b = Box<int>(1);
  final (int a, String s) = (1, '');
  if (o is List) {}
  if (o is Box) {}
  if (y case final int z) {}
  final int Function(int) g = (int v) => v;
  int x;
  x = 1;
  int p;
  (p, _) = (1, 2);
  print(<Object>[local, map, a, s, b, g, x, p]);
}

set value(int v) {}
