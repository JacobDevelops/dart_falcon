class Point {
  int x = 0;
  void reset() {}
  static void create() {}
  static void destroy() {}
}

void cascaded() {
  final list = <int>[]
    ..add(1)
    ..add(2);
  print(list);
}

void different(List<int> a, List<int> b) {
  a.add(1);
  b.add(2);
}

void nullAware(List<int>? list) {
  list?.add(1);
  list?.add(2);
}

void staticCalls() {
  Point.create();
  Point.destroy();
}

void dependency(List<int> list) {
  list.add(1);
  list.add(list.length);
}

void awaited() async {
  final values = await Future.value(<int>[]);
  values.add(1);
}

void constDeclaration() {
  const list = <int>[];
  list.length;
}

void ifNull(List<int>? list) {
  list ??= [];
  list.add(1);
}

void single(List<int> list) {
  list.add(1);
}

void separated(List<int> list) {
  list.add(1);
  print('x');
  list.add(2);
}

void chained(Map<String, List<int>> map) {
  map['a']!.add(1);
  map['a']!.add(2);
}
