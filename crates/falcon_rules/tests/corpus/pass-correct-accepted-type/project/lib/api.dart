import 'package:dart_code_metrics_annotations/annotations.dart';
export 'package:dart_code_metrics_annotations/annotations.dart';
class Animal {}
class Cat extends Animal {}
class Pet implements Animal {}
typedef Creature = Animal;

void accept(@AcceptedTypes({String, bool}) Object value) {}
void animal({@AcceptedTypes({Creature}) required Object value}) {}
class Box {
  @AcceptedTypes({Animal})
  final Object value;
  const Box(this.value);
  const Box.named(this.value);
  void send(@AcceptedTypes({String, bool}) Object value) {}
  static void shared(@AcceptedTypes({String}) Object value) {}
}
class Inherited extends Box {
  Inherited(Animal value) : super(value);
}

const accepted = {String, bool};
void constant(@AcceptedTypes(accepted) Object value) {}
void generic(@AcceptedTypes({List<String>}) Object value) {}
