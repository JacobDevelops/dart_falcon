import 'package:meta/meta.dart';

class Point {
  int x;
  Point(this.x);

  @override
  bool operator ==(Object other) => other is Point && other.x == x; /* expect: avoid-equals-and-hash-code-on-mutable-classes */

  @override
  int get hashCode => x.hashCode; /* expect: avoid-equals-and-hash-code-on-mutable-classes */
}

class Name {
  final String value;
  Name(this.value);

  @override
  bool operator ==(Object other) => other is Name && other.value == value; /* expect: avoid-equals-and-hash-code-on-mutable-classes */
}

class Id {
  final int id;
  Id(this.id);

  @override
  int get hashCode => id; /* expect: avoid-equals-and-hash-code-on-mutable-classes */
}

// A non-immutable superclass does not make the subclass immutable.
class Base {}

class Derived extends Base {
  @override
  bool operator ==(Object other) => identical(this, other); /* expect: avoid-equals-and-hash-code-on-mutable-classes */
}

mixin class Tagged {
  @override
  int get hashCode => 7; /* expect: avoid-equals-and-hash-code-on-mutable-classes */
}

// `@immutable` on an unrelated declaration does not apply.
@immutable
class Frozen {
  const Frozen();
}

class Unrelated {
  @override
  bool operator ==(Object other) => other is Unrelated; /* expect: avoid-equals-and-hash-code-on-mutable-classes */
}

// SDK supertypes never carry `@immutable`.
class Ordered implements Comparable<Ordered> {
  int value = 0;
  @override
  int compareTo(Ordered other) => 0;
  @override
  bool operator ==(Object other) => other is Ordered && other.value == value; /* expect: avoid-equals-and-hash-code-on-mutable-classes */
}
