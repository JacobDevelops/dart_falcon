abstract class CustomIterable implements Iterable<int> {
  @override
  int reduce(int Function(int, int) combine) => 0;
}

int customIterable(CustomIterable value) => value.reduce((a, b) => a + b);

int guardedElse(List<int> list) {
  if (list.isEmpty) {
    return 0;
  } else {
    return list.reduce((a, b) => a + b);
  }
}
