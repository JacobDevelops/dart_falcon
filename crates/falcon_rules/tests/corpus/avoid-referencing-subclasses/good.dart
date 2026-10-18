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
