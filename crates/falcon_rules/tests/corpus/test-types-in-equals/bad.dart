class Field {
  final int value;
  Field(this.value);

  @override
  bool operator ==(Object other) {
    Field otherField = other as Field; /* expect: test-types-in-equals */
    return value == otherField.value;
  }

  @override
  int get hashCode => value.hashCode;
}

class Pair {
  final int a;
  Pair(this.a);

  @override
  bool operator ==(Object other) => (other as Pair).a == a; /* expect: test-types-in-equals */

  @override
  int get hashCode => a;
}

class Named {
  final String name;
  Named(this.name);

  @override
  bool operator ==(Object o) =>
      identical(this, o) || (o as Named).name == name; /* expect: test-types-in-equals */

  @override
  int get hashCode => name.hashCode;
}

enum Color {
  red;

  bool same(Object other) => true;
}

mixin Comparable2 {
  int get key;

  @override
  bool operator ==(Object other) => key == (other as Comparable2).key; /* expect: test-types-in-equals */

  @override
  int get hashCode => key;
}

class Twice {
  final int x;
  Twice(this.x);

  @override
  bool operator ==(Object other) {
    final a = other as Twice; /* expect: test-types-in-equals */
    final b = other as dynamic; /* expect: test-types-in-equals */
    return a.x == b.x;
  }

  @override
  int get hashCode => x;
}
