class Base {
  Base? parent;
  Unrelated? value;
  Object factory(Object Child) => Child;
  T generic<T>(T value) => value;
  void shadow() {
    final Child = () => Object();
    Child();
  }
  Child genericShadow<Child>(Child value) => value;
}
class Child extends Base {}
class Unrelated {}
const freezed = 0;
@freezed
class GeneratedBase { GeneratedChild? child; }
class GeneratedChild extends GeneratedBase {}
class GenericBase<GenericChild> { GenericChild? value; }
class GenericChild extends GenericBase<int> {}

abstract class Repo { factory Repo() = RepoImpl; }
class RepoImpl implements Repo {}
class ConstraintBase { ConstrainedMixin? value; }
mixin ConstrainedMixin on ConstraintBase {}
class InterfaceBase { ViaInterface? value; }
class InterfaceOnly implements InterfaceBase {}
class ViaInterface extends InterfaceOnly {}

class Freezed { const Freezed({String? unionKey}); }
const unfreezed = 0;
@Freezed(unionKey: 'type')
class ConfiguredUnion { ConfiguredCase? value; }
class ConfiguredCase extends ConfiguredUnion {}
@unfreezed
class MutableUnion { MutableCase? value; }
class MutableCase extends MutableUnion {}
