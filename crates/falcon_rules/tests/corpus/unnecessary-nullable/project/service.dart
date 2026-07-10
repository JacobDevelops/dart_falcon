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
