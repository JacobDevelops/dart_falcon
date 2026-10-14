enum LogPriority {
  error(1, 'Error'),
  warning(2, 'Warning');

  final int priority;
  final String prefix;
  const LogPriority(this.priority, this.prefix);
}

// Only one constant.
class Single {
  static const only = Single._();
  const Single._();
}

// Public constructor on a public class.
class Open {
  static const a = Open(1);
  static const b = Open(2);
  final int v;
  const Open(this.v);
}

// Abstract classes are skipped.
abstract class Shape {
  static const circle = _Circle();
  static const square = _Circle();
  const Shape();
}

class _Circle extends Shape {
  const _Circle();
}

// Overrides `==` / `hashCode`.
class Coin {
  static const heads = Coin._(0);
  static const tails = Coin._(1);
  final int side;
  const Coin._(this.side);
  @override
  bool operator ==(Object other) => other is Coin && other.side == side;
  @override
  int get hashCode => side;
}

// Declares `index` or `values`.
class Indexed {
  static const first = Indexed._(0);
  static const second = Indexed._(1);
  final int index;
  const Indexed._(this.index);
}

// Subclassed in the library.
class Animal {
  static const cat = Animal._();
  static const dog = Animal._();
  const Animal._();
}

class Pet extends Animal {
  const Pet() : super._();
}

// Constructor invoked outside the constants.
class Level {
  static const low = Level._(0);
  static const high = Level._(1);
  final int value;
  const Level._(this.value);
}

Level custom(int v) => Level._(v);

// Non-const generative constructor.
class Mutable {
  static final a = Mutable._();
  static final b = Mutable._();
  Mutable._();
}

// Extends another class.
class Base {
  const Base();
}

class Derived extends Base {
  static const x = Derived._();
  static const y = Derived._();
  const Derived._();
}

// A public factory constructor on a public class.
class Suit {
  static const hearts = Suit._('h');
  static const spades = Suit._('s');
  final String code;
  const Suit._(this.code);
  factory Suit.parse(String code) => code == 'h' ? hearts : spades;
}
