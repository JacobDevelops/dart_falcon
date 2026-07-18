part of 'owner.dart';

// Writes the owner's private field — this is why `_count` cannot be final. Only
// visible when the rule unions writes across the whole library.
void resetCounter(Counter c) {
  c._count = 0;
}
