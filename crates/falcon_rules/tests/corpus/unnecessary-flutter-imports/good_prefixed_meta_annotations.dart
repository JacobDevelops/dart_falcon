import 'package:flutter/foundation.dart' as foundation show immutable, visibleForTesting;

@foundation.immutable
class ImmutableValue {
  const ImmutableValue();
  @foundation.visibleForTesting
  int get testValue => 1;
}
