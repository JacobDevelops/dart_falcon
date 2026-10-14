class A {
  int get height => 0;
  set height(int value) {}

  int width = 0;

  static int get count => 0;
  static set count(int value) {}
}

class Base {
  int get depth => 0;
  set name(String value) {}
  String get name => '';
}

class Sub extends Base {
  // Inherits the getter.
  set depth(int value) {}

  // Overrides an inherited setter.
  @override
  set name(String value) {}
}

class Mixed with M {
  set fromMixin(int value) {}
}

mixin M {
  int get fromMixin => 0;
}

// Mixins and extensions are not checked.
mixin OnlySetter {
  set value(int v) {}
}

extension Ext on int {
  set setterOnly(int v) {}
}
