class WildcardOwner<_> {
  var _;
  WildcardOwner.named(this._, _);
}
void wildcardFunction<_>(_) {}
typedef void WildcardAlias<_>(_);
typedef void Typed(int value);
typedef void Explicit(dynamic int);
typedef Modern = void Function(int);
typedef void Unknown(Undefined);
class Existing {}
typedef void ExplicitClass(Object Existing);
class Owner<T> {
  void member<T>(T value) {}
  static void staticMember<T>(T value) {}
}
