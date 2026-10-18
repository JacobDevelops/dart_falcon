int f(Object value, bool ready, int count) => switch (value) {
  int() when ready => 1,
  int() when ready => 2, /* expect: avoid-duplicate-switch-case-conditions */
  String() when count > 1 => 3,
  String() when count>1 => 4, /* expect: avoid-duplicate-switch-case-conditions */
  bool() when !ready => 5,
  bool() when !ready => 6, /* expect: avoid-duplicate-switch-case-conditions */
  double() when count == 2 => 7,
  double() when count == 2 => 8, /* expect: avoid-duplicate-switch-case-conditions */
  _ => 0,
};
void g(Object value, bool ready) {
  switch (value) {
    case int() when ready: break;
    case int() when ready: break; /* expect: avoid-duplicate-switch-case-conditions */
    default: break;
  }
}

int stringGuards(Object value, String text) => switch (value) {
  int() when text.isEmpty => 1,
  int() when text.isEmpty => 2, /* expect: avoid-duplicate-switch-case-conditions */
  String() when text.length > 2 => 3,
  String() when text.length > 2 => 4, /* expect: avoid-duplicate-switch-case-conditions */
  bool() when text.isNotEmpty => 5,
  bool() when text.isNotEmpty => 6, /* expect: avoid-duplicate-switch-case-conditions */
  _ => 0,
};
