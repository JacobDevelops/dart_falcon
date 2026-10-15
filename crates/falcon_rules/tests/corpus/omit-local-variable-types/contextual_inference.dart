class Cell<T> {
  Cell();
  T? value;
}

List<T> empty<T>() => [];
int get topCount => 1;
final topValue = 1;

class Parent<T> {
  final T value;
  Parent(this.value);
  T get first => value;
  T operator [](int index) => value;
}

class Child<T> extends Parent<T> {
  Child(super.value);
}

void contextual(bool flag, Child<int> child, int? optional) {
  Cell<int> cell = Cell(); /* expect: omit-local-variable-types */
  List<int> list = empty(); /* expect: omit-local-variable-types */
  int Function(int) callback = (value) => value; /* expect: omit-local-variable-types */
  List<int> conditional = flag ? [] : [1]; /* expect: omit-local-variable-types */
  int switched = switch (optional) { int value => value, _ => 0 }; /* expect: omit-local-variable-types */
  int indexed = child[0]; /* expect: omit-local-variable-types */
  int inherited = child.first; /* expect: omit-local-variable-types */
  int fallback = optional ?? 0; /* expect: omit-local-variable-types */
  int global = topValue; /* expect: omit-local-variable-types */
  int getter = topCount; /* expect: omit-local-variable-types */
  double negative = -1; /* expect: omit-local-variable-types */
  for (num item in [1]) {} /* expect: omit-local-variable-types */
  print([cell, list, callback, conditional, switched, indexed, inherited, fallback, global, getter, negative]);
}
