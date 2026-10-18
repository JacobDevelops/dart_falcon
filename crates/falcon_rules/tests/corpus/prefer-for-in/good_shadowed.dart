// A user-defined List need not be Iterable.
class List<T> {
  int get length => 3;
  int operator [](int index) => index;
}

void customList(List<int> items) {
  for (var i = 0; i < items.length; i++) {
    print(items[i]);
  }
}
