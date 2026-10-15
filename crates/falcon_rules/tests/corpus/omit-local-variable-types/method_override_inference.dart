// Override inference uses native combined members before initializer inference.
abstract class WideResult {
  Object result();
  void accept(int value);
  T echo<T>(T value);
  set value(int next);
  int operator +(int other);
}

abstract class NarrowResult {
  int result();
  void accept(num value);
}

class Combined implements WideResult, NarrowResult {
  result() => 1;
  accept(value) {
    final num copy = value; /* expect: omit-local-variable-types */
    print(copy);
  }
  echo<U>(value) {
    final U copy = value; /* expect: omit-local-variable-types */
    return copy;
  }
  set value(next) {
    final int copy = next; /* expect: omit-local-variable-types */
    print(copy);
  }
  operator +(other) {
    final int copy = other; /* expect: omit-local-variable-types */
    return copy;
  }
}

final topValue = Early().measured;
class Early extends Middle {
  late final measured = measure('');
  measure(text) => 0;
}
abstract class Middle extends Base {
  measure(text);
}
abstract class Base {
  int measure(String text);
}

extension Describe on Object {
  String describe(int value) => '';
}
class Unrelated {
  describe(value) => 1;
}

class Representation implements Base {
  int measure(String text) => 0;
}
extension type Wrapper(Representation representation) implements Base {
  measure(text) {
    final String copy = text;
    print(copy);
    return 0;
  }
}

void examples(Combined combined, Early early, Unrelated unrelated, Wrapper wrapper) {
  final int result = combined.result(); /* expect: omit-local-variable-types */
  final int measured = early.measured; /* expect: omit-local-variable-types */
  final int top = topValue; /* expect: omit-local-variable-types */
  final String notAnOverride = unrelated.describe(0);
  final int notInherited = wrapper.measure('');
  print([result, measured, top, notAnOverride, notInherited]);
}

class GetterBase {
  int get value => 0;
}
class GetterSetter extends GetterBase {
  set value(next) {
    final int copy = next; /* expect: omit-local-variable-types */
    print(copy);
  }
}
class FieldSetter extends FieldBase {
  set value(next) {
    final int copy = next; /* expect: omit-local-variable-types */
    print(copy);
  }
}
class FieldBase {
  var value = 0;
}
abstract class UnusedGenericBase {
  void run<T>(int value);
}
class UnusedGenericImpl implements UnusedGenericBase {
  run<U>(value) {
    final int copy = value; /* expect: omit-local-variable-types */
    print(copy);
  }
}
class Native {
  int method(String value) => 0;
  set value(int next) {}
}
extension Shadow on Native {
  method(value) {
    final String copy = value;
    print(copy);
    return 0;
  }
  set value(next) {
    final int copy = next;
    print(copy);
  }
}
