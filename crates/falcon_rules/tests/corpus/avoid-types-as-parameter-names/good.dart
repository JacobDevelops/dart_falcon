// @dart = 3.13

typedef Callback = void Function(int value);
void callback(Callback callback) {}
class Widget {}
void explicitFunction(void Function(int) int) {}
void explicitAlias(Object Callback) {}
void explicitClass(Object Widget) {}
class Box<T> {
  void method<T>() {}
}
void implicitFunction(int()) {}

// Declaring primary-constructor parameters have field-formal elements.
class DeclaringFinal(final Widget);
class DeclaringVar(var Widget);
class WildcardTypeParameter<_> {
  var _;
  WildcardTypeParameter(this._, _);
}
