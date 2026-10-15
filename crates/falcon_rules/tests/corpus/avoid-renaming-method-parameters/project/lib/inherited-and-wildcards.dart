// SDK-free ports of official mixinApplication and wildcard tests.
mixin ParentMixin {
  void method(int value) {}
}
abstract class Application = Object with ParentMixin;
class Child extends Application {
  void method(int renamed) {} /* expect: avoid-renaming-method-parameters */
}
class ObjectOverride {
  dynamic noSuchMethod(Invocation call) => null; /* expect: avoid-renaming-method-parameters */
}
class Base {
  void positional(int a, int b, int c) {}
  void wildcard(int _) {}
}
class Wildcards extends Base {
  void positional(int _, int renamed, int _) {} /* expect: avoid-renaming-method-parameters */
  void wildcard(int value) {}
}
class NonWildcards extends Base {
  void positional(int _a, int b, int c) {} /* expect: avoid-renaming-method-parameters */
  /// Official documented overrides are exempt.
  void wildcard(int renamed) {}
}
