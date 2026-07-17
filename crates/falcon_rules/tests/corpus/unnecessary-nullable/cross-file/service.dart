// Private declarations whose nullable params are (or aren't) genuinely nullable.
class Service {
  // Never passed null at any call site → the `?` is unnecessary.
  void _never(int? a) { /* expect: unnecessary-nullable */
    print(a);
  }

  // Passed null from caller.dart → correctly nullable, not flagged.
  void _sometimes(int? b) {
    print(b);
  }

  // Body assigns null → nullability is used, not flagged.
  void _assigns(String? c) {
    c = null;
    print(c);
  }

  // Ambiguous name (also declared in Other) → skipped entirely.
  void _ambiguous(int? d) {
    print(d);
  }
}

class Other {
  void _ambiguous(int? d) {
    print(d);
  }
}

// Private top-level function never passed null → flagged.
void _topNever(String? s) { /* expect: unnecessary-nullable */
  print(s);
}

// Return-typed helpers whose declared return type drives the null-flow analysis
// at the call sites in caller.dart.
int nonNull() => 1;
int? maybeNull() => null;

// Only ever passed a value the project index proves non-nullable (`nonNull()`
// returns a non-null `int`) → the `?` is unnecessary, so flagged.
void _viaProvenNonNull(int? v) { /* expect: unnecessary-nullable */
  print(v);
}

// Only ever passed a value whose declared return type is nullable
// (`maybeNull()` returns `int?`) → genuinely nullable, so NOT flagged. Literal-
// only matching would have wrongly flagged this before the resolver landed.
void _viaNullableReturn(int? v) {
  print(v);
}
