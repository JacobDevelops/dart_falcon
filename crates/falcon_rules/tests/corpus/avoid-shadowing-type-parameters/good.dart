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
