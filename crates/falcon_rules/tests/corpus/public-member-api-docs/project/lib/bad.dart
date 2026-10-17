import 'package:meta/meta.dart';

class Api { /* expect: public-member-api-docs */
  Api(); /* expect: public-member-api-docs */
  int field = 0; /* expect: public-member-api-docs */
  void method() {} /* expect: public-member-api-docs */
  int get value => 0; /* expect: public-member-api-docs */
  set value(int v) {} /* expect: public-member-api-docs */
}
void function() {} /* expect: public-member-api-docs */
int variable = 0; /* expect: public-member-api-docs */
typedef Alias = int; /* expect: public-member-api-docs */
enum Choice { /* expect: public-member-api-docs */
  one, /* expect: public-member-api-docs */
  two /* expect: public-member-api-docs */
}
mixin Features { /* expect: public-member-api-docs */
  void method() {} /* expect: public-member-api-docs */
}
extension Named on int { /* expect: public-member-api-docs */
  int get doubled => this * 2; /* expect: public-member-api-docs */
}
extension type Wrapper(int value) { /* expect: public-member-api-docs */
  static void utility() {} /* expect: public-member-api-docs */
}
int get setting => 0; /* expect: public-member-api-docs */
set setting(int value) {} /* expect: public-member-api-docs */
/// Members do not inherit their own `@internal`.
class Internals {
  @internal
  void hidden() {} /* expect: public-member-api-docs */
}
/// Declares only a setter.
abstract class SetterBase {
  /// Sink.
  set sink(int value);
}
/// A field does not override an inherited setter.
class SetterField extends SetterBase {
  int sink = 0; /* expect: public-member-api-docs */
}
/// A getter does not override an inherited setter.
class SetterGetter extends SetterBase {
  int get sink => 0; /* expect: public-member-api-docs */
  set sink(int value) {}
}

/// An annotation alone does not make a member an override.
class NonOverrides {
  @override
  void absent() {} /* expect: public-member-api-docs */
  @override
  int field = 0; /* expect: public-member-api-docs */
  @override
  int get value => 0; /* expect: public-member-api-docs */
  @override
  set value(int next) {} /* expect: public-member-api-docs */
  @override
  static void utility() {} /* expect: public-member-api-docs */
}

@override
void annotatedFunction() {} /* expect: public-member-api-docs */

/* Ordinary block comments are not documentation. */
class PlainComment {} /* expect: public-member-api-docs */

/// Setter diagnostics retain declaration order even when their getter comes later.
class SetterBeforeGetter {
  set later(int value) {} /* expect: public-member-api-docs */
  int get later => 0; /* expect: public-member-api-docs */
}

set laterTopLevel(int value) {} /* expect: public-member-api-docs */
int get laterTopLevel => 0; /* expect: public-member-api-docs */
