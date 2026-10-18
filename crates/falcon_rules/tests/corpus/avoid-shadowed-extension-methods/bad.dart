class Base {
  void inherited() {}
}

class Target extends Base {
  void run() {}
  int get size => 0;
  int field = 0;
  set setting(int value) {}
}

typedef Alias = Target;

extension Hidden on Target {
  void run() {} /* expect: avoid-shadowed-extension-methods */
  void inherited() {} /* expect: avoid-shadowed-extension-methods */
  void size() {} /* expect: avoid-shadowed-extension-methods */
  void field() {} /* expect: avoid-shadowed-extension-methods */
  int get setting => 1; /* expect: avoid-shadowed-extension-methods */
}

extension Aliased on Alias {
  void run() {} /* expect: avoid-shadowed-extension-methods */
}

extension Bounded<T extends Target> on T {
  void inherited() {} /* expect: avoid-shadowed-extension-methods */
}

extension on Target {
  set size(int value) {} /* expect: avoid-shadowed-extension-methods */
}
