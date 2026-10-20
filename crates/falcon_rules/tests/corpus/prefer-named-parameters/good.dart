void configure(int width, int height, {required String label}) {}
void optional(int width, [int height = 0]) {}
class Base {
  void resize(int width, int height) {}
}
class Child extends Base {
  @override
  void resize(int width, int height) {}
  void inheritedContext(int a, int b, int c) {}
}
final callback = (int a, int b, int c) {};
typedef Callback = void Function(int a, int b, int c);
void main(List<String> args, Object? message) {}
external int nativeFunction(int a, int b, int c);
@deprecated
void annotated(int a, int b, int c) {}
class Redirect {
  factory Redirect(int a, int b, int c) = Target;
}
class Target implements Redirect {
  @deprecated
  Target(int a, int b, int c);
}
class Parent {
  @deprecated
  Parent(int a, int b, int c);
}
class SuperChild extends Parent {
  SuperChild(super.a, super.b, super.c);
}
