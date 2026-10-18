class Parent {
  Parent();
  Parent.named();
  Parent.value(int value);
}
class Implicit extends Parent { Implicit(); }
class Named extends Parent { Named() : super.named(); }
class Argument extends Parent { Argument() : super.value(1); }
