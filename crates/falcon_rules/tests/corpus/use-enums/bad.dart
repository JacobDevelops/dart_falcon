class LogPriority { /* expect: use-enums */
  static const error = LogPriority._(1, 'Error');
  static const warning = LogPriority._(2, 'Warning');
  static const log = LogPriority._unknown('Log');

  final String prefix;
  final int priority;
  const LogPriority._(this.priority, this.prefix);
  const LogPriority._unknown(String prefix) : this._(-1, prefix);
}

// Private classes may use a public constructor.
class _Direction { /* expect: use-enums */
  static const north = _Direction(0);
  static const south = _Direction(180);
  final int degrees;
  const _Direction(this.degrees);
}

// A private factory constructor does not disqualify the class.
class Suit { /* expect: use-enums */
  static const hearts = Suit._('h');
  static const spades = Suit._('s');
  final String code;
  const Suit._(this.code);
  factory Suit._parse(String code) => code == 'h' ? hearts : spades;
  static Suit parse(String code) => Suit._parse(code);
}

// `new`-style creation and `extends Object` are fine.
class Planet extends Object { /* expect: use-enums */
  static const mercury = const Planet._(0.38);
  static const venus = const Planet._(0.91);
  final double gravity;
  const Planet._(this.gravity);
  double weight(double mass) => mass * gravity;
}

// Other members are allowed alongside the constants.
class Status { /* expect: use-enums */
  static const active = Status._('active');
  static const inactive = Status._('inactive');
  static const all = [active, inactive];
  final String label;
  const Status._(this.label);
  @override
  String toString() => label;
}
