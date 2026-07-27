const seed = 1;

class Traffic {
  const Traffic._(this.value);
  final int value;

  static const Traffic red = Traffic._(0);
  static const Traffic yellow = Traffic._(1);
  @deprecated
  static const Traffic amber = yellow;
  static const Traffic green = Traffic._(2);
}

void describe(Traffic light) {
  switch (light) { /* expect: exhaustive-cases */
    case Traffic.red:
      break;
    case Traffic.amber:
      break;
  }
}

class UntypedToken {
  const UntypedToken._(this.value);
  final Object value;

  static const first = const UntypedToken._(seed + 1);
  static const firstAlias = UntypedToken._(2);
  static const second = UntypedToken._((true ? 3 : 4, [1, 2], {'x': 1}));
}

void describeUntyped(UntypedToken token) {
  switch (token) { /* expect: exhaustive-cases */
    case UntypedToken.firstAlias:
      break;
  }
}
