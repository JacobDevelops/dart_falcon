import 'dart:core' as core;

abstract class Base {
  void method(core.int value);
}

class Example extends Base {
  @core.override
  void method(core.int unused) {}
}

const overrideAlias = core.override;

class AliasedOverride extends Base {
  @overrideAlias
  void method(core.int unused) {}
}
