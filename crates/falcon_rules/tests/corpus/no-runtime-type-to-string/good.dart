abstract class Base {
  @override
  String toString() => '$runtimeType';
}

mixin Describe {
  String describe() => runtimeType.toString();
}

class Checked {
  void check(int value) {
    assert(value > 0, '$runtimeType: value must be positive');
    if (value > 10) {
      throw StateError('${runtimeType.toString()} overflow');
    }
    try {
      print(value);
    } catch (e) {
      print('$runtimeType failed: $e');
    }
  }

  String other(Object value) => value.runtimeType.toString();

  String plain() => toString();
}

class Shadowed {
  String describe(String runtimeType) => runtimeType.toString();
}

String topLevel(Object value) => '${value.runtimeType}';

extension on int {
  String tag() => runtimeType.toString();
}
