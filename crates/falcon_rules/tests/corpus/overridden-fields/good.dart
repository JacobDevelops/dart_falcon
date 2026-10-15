abstract class Base { covariant num value; }
class Child implements Base { int value = 1; }
class Other extends Base { int another = 0; }

class InterfaceField { final int viaInterface = 0; }
class Implementer implements InterfaceField {
  @override
  final int viaInterface = 1;
}

abstract class AbstractField { abstract final int declared; }
mixin AbstractFieldMixin on Object { abstract final int fromMixin; }
class Concrete extends AbstractField with AbstractFieldMixin {
  @override
  final int declared = 1;
  @override
  final int fromMixin = 2;
}

class FieldBase { final int shadowed = 0; }
class GetterMiddle extends FieldBase {
  @override
  int get shadowed => 1;
}
class ShadowedByGetter extends GetterMiddle {
  @override
  final int shadowed = 2;
}
