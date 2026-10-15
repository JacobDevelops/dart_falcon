import 'dart:core';
import 'dart:core' as core;

class Deprecated {
  const Deprecated(String message);
}

@Deprecated('local metadata is not a deprecation')
class LocalMetadata {
  LocalMetadata();
}

@core.Deprecated('old')
class CoreMetadata {
  @core.deprecated
  CoreMetadata();
}

class FieldMetadata {
  FieldMetadata(@Deprecated('local') this.value);
  final int value;
}
