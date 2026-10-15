// @dart = 3.13

class A {
  int x = 0;
}
class B(var int x) extends A {} /* expect: overridden-fields */
