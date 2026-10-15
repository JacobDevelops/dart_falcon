import 'package:meta/meta.dart';
import 'package:meta/meta.dart' as m;

@immutable
class Point {
  final int x;
  const Point(this.x);

  @override
  bool operator ==(Object other) => other is Point && other.x == x;

  @override
  int get hashCode => x.hashCode;
}

// Immutability is inherited from an annotated supertype.
@immutable
abstract class Value {
  const Value();
}

class Meters extends Value {
  final double meters;
  const Meters(this.meters);

  @override
  bool operator ==(Object other) => other is Meters && other.meters == meters;

  @override
  int get hashCode => meters.hashCode;
}

class Kilometers implements Value {
  final double km;
  const Kilometers(this.km);

  @override
  bool operator ==(Object other) => other is Kilometers && other.km == km;

  @override
  int get hashCode => km.hashCode;
}

// Mutable classes that keep identity equality are fine.
class Counter {
  int count = 0;
  void increment() => count++;
}

// Other operators are not reported.
class Vector {
  int x = 0;
  Vector operator +(Vector other) => Vector()..x = x + other.x;
}

// Extensions cannot override `==`; enums cannot either.
enum Mode { on, off }

@m.Immutable('x')
class ImmutableReason {
  @override
  bool operator ==(Object other) => true;
}

@Immutable()
class ImmutableConstructor {
  @override
  int get hashCode => 0;
}
