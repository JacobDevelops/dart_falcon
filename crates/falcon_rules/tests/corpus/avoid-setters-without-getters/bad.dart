class A {
  set width(int value) {} /* expect: avoid-setters-without-getters */

  static set registry(Object value) {} /* expect: avoid-setters-without-getters */
}

enum Mode {
  on, off;

  set label(String value) {} /* expect: avoid-setters-without-getters */
}

extension type Wrapper(int raw) {
  set doubled(int value) {} /* expect: avoid-setters-without-getters */
}

class Base {
  int get height => 0;
}

class Sub extends Base {
  set depth(int value) {} /* expect: avoid-setters-without-getters */
}

class WithMethod {
  void size() {}
  set length(int value) {} /* expect: avoid-setters-without-getters */
}
