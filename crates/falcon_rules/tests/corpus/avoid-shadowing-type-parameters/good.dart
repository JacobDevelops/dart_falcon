// Nested type parameters use distinct names.

class A<T> {
  void m<S>() {}
}

class B<T> {
  void m() {}
}

void f<T>() {
  void g<S>() {}
  g<int>();
}

typedef Fn<T> = void Function(T value);

class C<T, S> {
  S convert(T x) => x as S;
}

void h<T>(T x) {
  print(x);
}

class D {
  void m<T>() {}
}

class StaticClass<T> {
  static void method<T>() {}
}

mixin StaticMixin<T> {
  static void method<T>() {}
}

enum StaticEnum<T> {
  value<int>();
  static void method<T>() {}
}

extension StaticExtension<T> on List<T> {
  static void method<T>() {}
}

extension type StaticRepresentation<T>(List<T> values) {
  static void method<T>() {}
}
