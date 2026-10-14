/// Documented class.
class Api {
  /// Constructor.
  Api();
  /** Field. */
  int field = 0;
  /// Getter documents the setter too.
  int get value => 0;
  set value(int v) {}
  set standalone(int v) {}
  @override
  String toString() => '';
  void _private() {}
}
class _Private { void publicMethod() {} }
void main() {}
@internal
class Hidden { void hidden() {} }
/// Documented function.
void documented() {}
/// Alias.
typedef Alias = int;
/// Mixin.
mixin Features {
  /// Method.
  void method() {}
}
/// Extension.
extension Named on int {
  /// Getter.
  int get doubled => this * 2;
}
/// Base.
class Base {
  /// Method.
  void inherited() {}
  /// Getter.
  int get value => 0;
}
/// Derived.
class Derived extends Base {
  void inherited() {}
  int get value => 1;
}
/// Unknown inheritance is not proof of a new member.
class Unknown extends Unresolved {
  void uncertain() {}
}
/// The getter documents its paired setter.
int get setting => 0;
set setting(int value) {}
set standalone(int value) {}
/// Overrides of SDK members are not new API.
class Version implements Comparable<Version> {
  int compareTo(Version other) => 0;
}
/// Iterator members come from the SDK.
class MyIterator implements Iterator<int> {
  int get current => 0;
  bool moveNext() => false;
}
