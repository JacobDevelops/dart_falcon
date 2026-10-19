void exits(Iterable<int> values) {
  for (final value in values) { print(value); break; } /* expect: avoid-unconditional-break */
  for (final value in values) { print(value); continue; } /* expect: avoid-unconditional-break */
  for (final value in values) { print(value); return; } /* expect: avoid-unconditional-break */
  for (final value in values) { throw value; } /* expect: avoid-unconditional-break */
  for (var i = 0; i < 10; i++) { final value = i; print(value); break; } /* expect: avoid-unconditional-break */
}
void bare(Iterable<int> values) {
  for (final value in values) continue; /* expect: avoid-unconditional-break */
}
