class Base {
  Child? field; /* expect: avoid-referencing-subclasses */
  Child make() => throw 0; /* expect: avoid-referencing-subclasses */
  void accept(Child value) {} /* expect: avoid-referencing-subclasses */
  List<Grandchild> children = []; /* expect: avoid-referencing-subclasses */
  Object create() => Child(); /* expect: avoid-referencing-subclasses */
  int staticValue() => Child.value; /* expect: avoid-referencing-subclasses */
  bool test(Object value) => value is Child; /* expect: avoid-referencing-subclasses */
}
class Child extends Base { static int value = 1; }
class Grandchild extends Child {}
