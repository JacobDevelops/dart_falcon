class Counter {
  static int count = 0;
  static int? cached;
  void update() {
    count = 1; /* expect: avoid-assigning-to-static-field */
    count += 2; /* expect: avoid-assigning-to-static-field */
    count++; /* expect: avoid-assigning-to-static-field */
    --count; /* expect: avoid-assigning-to-static-field */
    cached ??= 1; /* expect: avoid-assigning-to-static-field */
    Counter.count = 2; /* expect: avoid-assigning-to-static-field */
    Counter.count *= 2; /* expect: avoid-assigning-to-static-field */
    Counter.count--; /* expect: avoid-assigning-to-static-field */
  }
}
