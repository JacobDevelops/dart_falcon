// With `dart:core` imported only under a prefix, a bare `dynamic` does not
// resolve (an analyzer error), so it is not a `dynamic` annotation.
import 'dart:core' as core;

void unresolved(dynamic value) {}

void typed(core.Object? value) {}
