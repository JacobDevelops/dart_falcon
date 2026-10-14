class Box {
  Box(this.value);
  Box.named(this.value);
  final int value;
}

class Node {
  void add(Node other) {}
}

void log(Object? value) {}

void main(List<int> values, List<Node> nodes, int other) {
  values.forEach(print);
  for (final v in values) {
    print(v);
    print(v);
  }
  for (final v in values) print(other);
  for (final v in values) log(v + 1);
  for (final v in values) Box(v);
  for (final v in values) Box.named(v);
  for (final n in nodes) n.add(n);
  int x = 0;
  for (x in values) print(x);
  for (final (a, b) in [(1, 2)]) print(a);
  for (var i = 0; i < 3; i++) print(i);
  for (final v in values) unknownFunction(v);
}
