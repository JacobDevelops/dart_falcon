String name() => '';
class C {
  T id<T>(T value) => value;
  static T keep<T>(T value) => value;
  static int name() => 0;
  void instance(List<int> values) {
    final Iterable<int> direct = values.map(id); /* expect: omit-local-variable-types */
    final List<int> chained = values.map(id).toList(); /* expect: omit-local-variable-types */
    final int called = name(); /* expect: omit-local-variable-types */
    final int Function() torn = name; /* expect: omit-local-variable-types */
  }
  static void statics(List<int> values) {
    final Iterable<int> direct = values.map(keep); /* expect: omit-local-variable-types */
  }
}
extension Conflict on C { String name() => ''; }
class Base { T identity<T>(T value) => value; }
class Derived extends Base {
  void inherited(List<int> values) {
    final Iterable<int> direct = values.map(identity); /* expect: omit-local-variable-types */
  }
}
class A { int method({int a = 0, int b = 0, int c = 0}) => 0; }
class Z { int method({int c = 0, int b = 0, int a = 0}) => 0; }
void named(bool condition, A a, Z z) {
  final int Function({int a, int b, int c}) callback = condition ? a.method : z.method; /* expect: omit-local-variable-types */
}
void oldStyle(int callback({int b, int a})) {
  final int Function({int a, int b}) ordered = callback; /* expect: omit-local-variable-types */
}
