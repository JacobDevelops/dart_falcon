int shared = 1; /* expect: avoid-top-level-member-access */

extension Values on int {
  static int mutable = 2; /* expect: avoid-top-level-member-access */

  Object read() => (
    shared, /* expect: avoid-top-level-member-access */
    [if (this > 0) shared], /* expect: avoid-top-level-member-access */
  );
}
