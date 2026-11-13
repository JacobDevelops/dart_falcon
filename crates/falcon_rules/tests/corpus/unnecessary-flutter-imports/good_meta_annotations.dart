import 'package:flutter/foundation.dart';

@immutable
class ImmutableValue {
  const ImmutableValue();
  @visibleForTesting
  int get testValue => 1;
  @protected
  void protectedMethod() {}
}
