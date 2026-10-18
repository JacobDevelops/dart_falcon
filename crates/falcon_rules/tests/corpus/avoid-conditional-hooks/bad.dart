import 'package:flutter_hooks/flutter_hooks.dart';

int useCounter() => useState(0);

void useConditional(bool enabled, List<int> values) {
  if (enabled) {
    useState(0); /* expect: avoid-conditional-hooks */
  } else {
    useMemoized(() => 1); /* expect: avoid-conditional-hooks */
  }
  final first = enabled
      ? useState(1) /* expect: avoid-conditional-hooks */
      : useState(2); /* expect: avoid-conditional-hooks */
  final and = enabled && useState(true); /* expect: avoid-conditional-hooks */
  final or = enabled || useState(false); /* expect: avoid-conditional-hooks */
  int? optional;
  final fallback = optional ?? useState(1); /* expect: avoid-conditional-hooks */
  optional ??= useState(2); /* expect: avoid-conditional-hooks */
  for (final value in values) {
    useState(value); /* expect: avoid-conditional-hooks */
  }
  while (enabled) {
    useMemoized(() => 1); /* expect: avoid-conditional-hooks */
    break;
  }
  do {
    useState(0); /* expect: avoid-conditional-hooks */
  } while (enabled);
  for (var index = 0; index < values.length; index++) {
    useCounter(); /* expect: avoid-conditional-hooks */
  }
  final list = [if (enabled) useState(3)]; /* expect: avoid-conditional-hooks */
  final mapped = [for (final value in values) useState(value)]; /* expect: avoid-conditional-hooks */
  final map = {if (enabled) 'value': useState(1)}; /* expect: avoid-conditional-hooks */
  final repeatedMap = {for (final value in values) value: useState(value)}; /* expect: avoid-conditional-hooks */
  switch (first) {
    case 1:
      useState(0); /* expect: avoid-conditional-hooks */
      break;
  }
  final selected = switch (first) {
    1 => useState(1), /* expect: avoid-conditional-hooks */
    _ => 0,
  };
}
