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

class LaterShadow {
  String describe() => '$runtimeType';

  void unrelated() {
    final runtimeType = 'local';
    print(runtimeType);
  }
}

class InitializerExemptions {
  InitializerExemptions()
      : assert(runtimeType.toString().isNotEmpty, '$runtimeType');
}

// INVALID-CODE: annotation arguments need not be constant for this syntactic rule.
class IgnoredConstructorMetadata {
  @Tag(this.runtimeType.toString())
  IgnoredConstructorMetadata();

  factory IgnoredConstructorMetadata.redirect() =
      Target<void Function(@Tag(this.runtimeType.toString()) int value)>;
}
