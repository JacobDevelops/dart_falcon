void main() {
  final box = Box();
  print(box + 1);
  print(box[0]);
  print(-box);
  print(box.extra());
  box.counter += 1;
  print(LocalIterable());
}

class Box {
  int counter = 0;
  Box operator +(int value) => this;
  int operator [](int index) => index;
  Box operator -() => this;
}

extension Extra on Box {
  int extra() => 1;
}

class LocalIterable extends Iterable<int> {
  @override
  Iterator<int> get iterator => [1].iterator;
}
