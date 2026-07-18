// A nested declaration's type parameter must not shadow an enclosing one.

class A<T> {
  void m<T>() {} /* expect: avoid-shadowing-type-parameters */
}

class B<T, S> {
  S f<S>(S x) => x; /* expect: avoid-shadowing-type-parameters */
}

mixin M<T> {
  void p<T>() {} /* expect: avoid-shadowing-type-parameters */
}

void g<T>() {
  void h<T>() {} /* expect: avoid-shadowing-type-parameters */
  h<int>();
}

class C<E> {
  void m() {
    void inner<E>() {} /* expect: avoid-shadowing-type-parameters */
    inner<int>();
  }
}

extension Ext<T> on List<T> {
  void each<T>() {} /* expect: avoid-shadowing-type-parameters */
}
