import 'package:dart_code_metrics_annotations/annotations.dart';

void accept(@AcceptedTypes({String, bool}) Object value) {}
void named({@AcceptedTypes({String, bool}) required Object value}) {}
void optional([@AcceptedTypes({String, bool}) Object value = '']) {}

class Box {
  @AcceptedTypes({String, int})
  final Object value;
  const Box(this.value);
  const Box.named(this.value);
  void accept(@AcceptedTypes({String, bool}) Object value) {}
  static void send(@AcceptedTypes({String, bool}) Object value) {}
}

void example() {
  accept(1); /* expect: pass-correct-accepted-type */
  named(value: 2); /* expect: pass-correct-accepted-type */
  optional(3); /* expect: pass-correct-accepted-type */
  Box(false); /* expect: pass-correct-accepted-type */
  new Box(false); /* expect: pass-correct-accepted-type */
  const Box.named(false); /* expect: pass-correct-accepted-type */
  Box(1).accept(4); /* expect: pass-correct-accepted-type */
  Box.send(5); /* expect: pass-correct-accepted-type */
  Box(1)..accept(6); /* expect: pass-correct-accepted-type */
}

class Tag {
  const Tag(@AcceptedTypes({String, bool}) Object value);
}
@Tag(1) /* expect: pass-correct-accepted-type */
class Tagged {}
enum Choice {
  first(1); /* expect: pass-correct-accepted-type */
  const Choice(@AcceptedTypes({String, bool}) Object value);
}
void never(@AcceptedTypes({}) Object value) {}
void impossible() {
  never(1); /* expect: pass-correct-accepted-type */
}

class Child extends Box {
  Child(super.value);
  Child.p(super.value) : super.named();
}
class Grandchild extends Child {
  Grandchild(super.value);
}
class NamedBox {
  NamedBox({@AcceptedTypes({String}) required Object value});
}
class NamedChild extends NamedBox {
  NamedChild({required super.value});
}
void inheritedFormals() {
  Child(false); /* expect: pass-correct-accepted-type */
  Child.p(false); /* expect: pass-correct-accepted-type */
  Grandchild(false); /* expect: pass-correct-accepted-type */
  NamedChild(value: false); /* expect: pass-correct-accepted-type */
  Box b = .new(false); /* expect: pass-correct-accepted-type */
  Box c = .named(false); /* expect: pass-correct-accepted-type */
}

class Parent {
  Parent(@AcceptedTypes({String}) Object value);
  Parent.named(@AcceptedTypes({int}) Object value);
}
class Selected extends Parent {
  Selected(super.value);
  Selected.named(super.value) : super.named();
}
class Offset extends Parent {
  Offset(int extra, super.value);
}
void additionalInheritance() {
  Selected(1); /* expect: pass-correct-accepted-type */
  Selected.named('text'); /* expect: pass-correct-accepted-type */
  Offset(1, false); /* expect: pass-correct-accepted-type */
}
