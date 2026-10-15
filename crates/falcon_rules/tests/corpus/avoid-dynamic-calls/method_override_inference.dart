// Declaration contexts are lexical and do not require generic occurrences.
class OwnGeneric {
  unused<T>(value) {
    value.abs(); /* expect: avoid-dynamic-calls */
  }
}
abstract class GenericBase {
  void run<T>(int value);
}
class GenericImpl implements GenericBase {
  run<U>(value) {
    value.abs();
  }
}
class GetterBase {
  int get value => 0;
}
class GetterSetter extends GetterBase {
  set value(next) {
    next.abs();
  }
}
class FieldSetter extends FieldBase {
  set value(next) {
    next.abs();
  }
}
class FieldBase {
  var value = 0;
}
class Native {
  void method(int value) {}
  set value(int next) {}
}
extension Shadow on Native {
  method(value) {
    value.abs(); /* expect: avoid-dynamic-calls */
  }
  set value(next) {
    next.abs(); /* expect: avoid-dynamic-calls */
  }
}

class Version implements Comparable<Version> {
  final int major;
  Version(this.major);
  compareTo(other) => major - other.major;
}
