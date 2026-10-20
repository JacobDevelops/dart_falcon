int sum(int a, int b) => a + b;

void unsafe(List<int> list, Set<int> set, Iterable<int> iterable, List<int>? nullable) {
  list.reduce(sum); /* expect: avoid-unsafe-reduce */
  set.reduce((a, b) => a + b); /* expect: avoid-unsafe-reduce */
  iterable.reduce(sum); /* expect: avoid-unsafe-reduce */
  nullable?.reduce(sum); /* expect: avoid-unsafe-reduce */
  <int>[].reduce(sum); /* expect: avoid-unsafe-reduce */
  list..reduce(sum); /* expect: avoid-unsafe-reduce */
  if (list.isNotEmpty) {
    list.clear();
    list.reduce((a, b) => a + b); /* expect: avoid-unsafe-reduce */
  }
}

int mutatingCallback(List<int> list) {
  int Function(int, int) callback() {
    list.clear();
    return sum;
  }
  return list.isNotEmpty ? list.reduce(callback()) : 0; /* expect: avoid-unsafe-reduce */
}

var reads = 0;
List<int> get values => reads++ == 0 ? [1] : [];
int unstableGetter() =>
    values.isNotEmpty ? values.reduce((a, b) => a + b) : 0; /* expect: avoid-unsafe-reduce */

void mutatedCascades() {
  [1, 2]..clear()..reduce(sum); /* expect: avoid-unsafe-reduce */
  [1, 2]..length = 0..reduce(sum); /* expect: avoid-unsafe-reduce */
}

class UnstableReceiver {
  List<int> values = [];
  List<int> get items => [];
  int field() =>
      values.isNotEmpty ? values.reduce(sum) : 0; /* expect: avoid-unsafe-reduce */
  int getter() =>
      items.isNotEmpty ? items.reduce(sum) : 0; /* expect: avoid-unsafe-reduce */
}

class StaticReceiver {
  static List<int> values = [];
  int sum() =>
      values.isNotEmpty ? values.reduce((a, b) => a + b) : 0; /* expect: avoid-unsafe-reduce */
}

abstract class AbstractReceiver {
  abstract final List<int> values;
  int sum() =>
      values.isNotEmpty ? values.reduce((a, b) => a + b) : 0; /* expect: avoid-unsafe-reduce */
}

extension EmptyingRead on List<int> {
  int get empty {
    clear();
    return 0;
  }
}

void getterCascade() {
  [1]..empty..reduce(sum); /* expect: avoid-unsafe-reduce */
  [1, 2]..length..reduce(sum); /* expect: avoid-unsafe-reduce */
}
