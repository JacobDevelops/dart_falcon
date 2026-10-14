class Person {
  final String? name;
  Person(this.name);

  @override
  bool operator ==(Object? other) =>
      other != null && /* expect: avoid-null-checks-in-equality-operators */
      other is Person &&
      name == other.name;

  @override
  int get hashCode => name.hashCode;
}

class Pet {
  final String? name;
  Pet(this.name);

  @override
  bool operator ==(Object? other) {
    if (null == other) return false; /* expect: avoid-null-checks-in-equality-operators */
    return other is Pet && name == other.name;
  }

  @override
  int get hashCode => name.hashCode;
}

class Car {
  final String? model;
  Car(this.model);

  @override
  bool operator ==(dynamic other) =>
      other?.model == model; /* expect: avoid-null-checks-in-equality-operators */

  @override
  int get hashCode => model.hashCode;
}

class Box {
  final Object? content;
  Box(this.content);

  @override
  bool operator ==(Object? other) {
    final value = other ?? this; /* expect: avoid-null-checks-in-equality-operators */
    return value is Box && value.content == content;
  }

  @override
  int get hashCode => content.hashCode;
}

class Tag {
  final String label;
  Tag(this.label);

  @override
  bool operator ==(Object? other) =>
      other?.toString() == label; /* expect: avoid-null-checks-in-equality-operators */

  @override
  int get hashCode => label.hashCode;
}
