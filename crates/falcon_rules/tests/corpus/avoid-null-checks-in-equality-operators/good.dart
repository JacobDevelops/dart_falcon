class Person {
  final String? name;
  Person(this.name);

  @override
  bool operator ==(Object other) => other is Person && name == other.name;

  @override
  int get hashCode => name.hashCode;
}

// Comparing a field with null is fine.
class Pet {
  final String? name;
  Pet(this.name);

  @override
  bool operator ==(Object other) => other is Pet && name != null && name == other.name;

  @override
  int get hashCode => name.hashCode;
}

// A rebound name refers to something else.
class Wrapper {
  final Object? value;
  Wrapper(this.value);

  @override
  bool operator ==(Object? other) {
    if (other is! Wrapper) return false;
    final inner = other.value;
    return [inner].any((other) => other != null);
  }

  @override
  int get hashCode => value.hashCode;
}

// Non-nullable parameter: `other` can never be null there anyway.
class Cell {
  final int? v;
  Cell(this.v);

  @override
  bool operator ==(Object other) => other is Cell && (v ?? 0) == (other.v ?? 0);

  @override
  int get hashCode => v.hashCode;
}

// Null checks in other operators and methods are not reported.
class Num {
  final int n;
  Num(this.n);

  bool equals(Object? other) => other != null && other is Num && other.n == n;

  Num operator +(Num? other) => Num(n + (other?.n ?? 0));
}
class Car {
  final String? model;
  Car(this.model);

  @override
  bool operator ==(dynamic other) =>
      other?.model == model;

  @override
  int get hashCode => model.hashCode;
}
