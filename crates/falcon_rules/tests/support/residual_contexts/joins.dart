// Numeric operator result typing depends on both operands.
void arithmetic(num number, double fraction, int integer) {
  final double sum = number + fraction; /* expect: omit-local-variable-types */
  final double difference = number - fraction; /* expect: omit-local-variable-types */
  final double product = number * fraction; /* expect: omit-local-variable-types */
  final double remainder = number % fraction; /* expect: omit-local-variable-types */
  final num wider = number + integer; /* expect: omit-local-variable-types */
  final num deliberatelyWider = number + fraction;
}

Future<T?> invoke<T>(String method) async => null;
Future<void> awaited() async {
  final Map<String, dynamic>? map = await invoke('map'); /* expect: omit-local-variable-types */
  final bool enabled = await invoke('enabled') ?? false; /* expect: omit-local-variable-types */
}

class Issue {}
class Node { int get role => 0; }
sealed class Checks {
  static Issue? check(Node node) {
    final Issue? issue = switch (node.role) { 0 => none, _ => some }(node); /* expect: omit-local-variable-types */
    return issue;
  }
  static Issue? none(Node node) => null;
  static Issue? some(Node node) => Issue();
}

class Parent {}
class Child extends Parent {}
class Box<T> {}
Parent up(Parent value) => value;
Child down(Child value) => value;
Parent acceptsChild(Child value) => value;
void variance(bool condition) {
  final Parent Function(Child) conditional = condition ? up : down; /* expect: omit-local-variable-types */
  final Parent Function(Child) selected = switch (condition) { true => up, false => down }; /* expect: omit-local-variable-types */
  final Parent Function(Child) widerReturn = condition ? down : acceptsChild; /* expect: omit-local-variable-types */
  final Box<Parent> inherited = condition ? Box<Child>() : Box<Parent>(); /* expect: omit-local-variable-types */
}

void nullableParameter<T extends Object>(T? value) {
  if (value != null) {
    final T copy = value; /* expect: omit-local-variable-types */
  }
}
void intersection<T>(T value) {
  if (value is int) {
    final int copy = value;
  }
}
void objectIntersection<T extends Object>(T value) {
  if (value is int) {
    final int copy = value;
  }
}

class Raw<T> {}
extension type Extension<T>(int value) {}
class Sequence<T> { Sequence.empty(); }
void rawTypes() {
  Raw raw = Raw(); /* expect: omit-local-variable-types */
  Extension value = Extension(1); /* expect: omit-local-variable-types */
  Sequence first = Sequence.empty(), second = Sequence.empty(); /* expect: omit-local-variable-types */
}
