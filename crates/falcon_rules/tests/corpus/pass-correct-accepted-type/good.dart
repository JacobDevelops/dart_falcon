import 'package:dart_code_metrics_annotations/annotations.dart';

class Animal {}
class Cat extends Animal {}
class Pet implements Animal {}
void accept(@AcceptedTypes({String, bool}) Object value) {}
void animal(@AcceptedTypes({Animal}) Object value) {}
void named({@AcceptedTypes({String, bool}) required Object value}) {}
void optional([@AcceptedTypes({String, bool}) Object value = '']) {}
void unrestricted(Object value) {}

class Box {
  @AcceptedTypes({String, int})
  final Object value;
  const Box(this.value);
  const Box.named(this.value);
  void accept(@AcceptedTypes({String, bool}) Object value) {}
}

void example(dynamic unknown) {
  accept('text');
  accept(true);
  animal(Animal());
  animal(Cat());
  animal(Pet());
  named(value: false);
  optional();
  optional('text');
  Box(1);
  new Box('text');
  const Box.named(2);
  Box(1)..accept(false);
  unrestricted(1);
  accept(unknown);
}

class Tag {
  const Tag(@AcceptedTypes({String, bool}) Object value);
}
@Tag('text')
class Tagged {}
enum Choice {
  first(false);
  const Choice(@AcceptedTypes({String, bool}) Object value);
}
typedef Accept = AcceptedTypes;
void aliased(@Accept({String}) Object value) {}
void aliasExample() {
  aliased('text');
}

void contextualDouble(@AcceptedTypes({double}) double value) {}

class ContextualDouble {
  @AcceptedTypes({double})
  final double value;
  ContextualDouble(this.value);
  void accept(@AcceptedTypes({double}) double value) {}
}

void contextualExamples() {
  contextualDouble(1);
  contextualDouble(1.5);
  ContextualDouble(1).accept(1);
}

class Child extends Box {
  Child(super.value);
  Child.p(super.value) : super.named();
}
void inheritedFormals() {
  Child('text');
  Child.p(1);
  Box b = .new(1);
  Box c = .named('text');
}

class GenericParent<T> {
  GenericParent(@AcceptedTypes({List<int>}) T value);
}
class IntListChild extends GenericParent<List<int>> {
  IntListChild(super.value);
}
void genericInheritance() {
  IntListChild([]);
}
