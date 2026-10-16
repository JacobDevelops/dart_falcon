int limit = 10;

class Range {
  final int start;
  final int end;
  Range(this.start, this.end) {
    assert(start <= end); /* expect: prefer-asserts-in-initializer-lists */
    assert(end < limit); /* expect: prefer-asserts-in-initializer-lists */
    print(start);
  }
  Range.of(int a, int b) : start = a, end = b {
    assert(a <= b, 'ordered'); /* expect: prefer-asserts-in-initializer-lists */
  }
  Range.single(int v) : start = v, end = v {
    assert(isValid(v)); /* expect: prefer-asserts-in-initializer-lists */
  }
  Range.list(List<int> items) : start = items.first, end = items.last {
    assert(items.every((e) => e >= 0)); /* expect: prefer-asserts-in-initializer-lists */
  }
  static bool isValid(int v) => v >= 0;
}

class Sub extends Range {
  Sub(int a) : super(a, a) {
    assert(a > 0); /* expect: prefer-asserts-in-initializer-lists */
  }
}

class Parent {
  final int value;
  Parent(this.value);
}

class SuperFormal extends Parent {
  SuperFormal(super.value) {
    assert(value > 0); /* expect: prefer-asserts-in-initializer-lists */
  }
}

extension type CheckedValue(int? value) {
  CheckedValue.checked(this.value) {
    assert(value != null); /* expect: prefer-asserts-in-initializer-lists */
  }
}

class ParameterMessage {
  ParameterMessage(int value) {
    assert(value > 0, value); /* expect: prefer-asserts-in-initializer-lists */
  }
}
