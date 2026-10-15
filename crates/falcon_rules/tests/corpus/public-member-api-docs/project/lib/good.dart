import 'package:meta/meta.dart';

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
/// Constructors of sealed classes are not public API.
sealed class Shape {
  const Shape();
}
/// Nor are those of `abstract final` classes.
abstract final class Registry {
  Registry.create();
}
/// Nor are those of `abstract interface` classes.
abstract interface class Port {
  Port.open();
}
/// Doc comments may follow or interleave the metadata.
class Annotated {
  @Deprecated('use other')
  @pragma('vm:prefer-inline')
  /// Documented after its metadata.
  int after = 0;
  @Deprecated('use other')
  /// Documented between annotations.
  @pragma('vm:prefer-inline')
  int between = 0;
  @Deprecated('use other')
  /// Documented before a plain comment.
  // Plain comment.
  int plain = 0;
}
/// Declares a getter.
abstract class GetterBase {
  /// Value.
  int get value;
}
/// A field overrides an inherited getter.
class GetterField extends GetterBase {
  final int value = 0;
}

/**/
class EmptyDocumentation {
  /**/
  EmptyDocumentation();
  /**/
  int field = 0;
  /**/
  void method() {}
  @Deprecated('old')
  /**/
  int get value => 0;
  set value(int next) {}
}

/**/
void emptyDocumentation() {}
