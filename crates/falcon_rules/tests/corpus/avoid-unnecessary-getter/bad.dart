class Arrow {
  final int _value = 1;
  int get /* expect: avoid-unnecessary-getter */ value => _value;
}
class Body {
  final int _other = 1;
  int get /* expect: avoid-unnecessary-getter */ other { return this._other; }
}
class Annotated {
  final int _hashCode = 1;
  @override
  int get /* expect: avoid-unnecessary-getter */ hashCode => _hashCode;
}
