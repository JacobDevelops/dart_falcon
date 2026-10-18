class Upcast {
  final int _value = 1;
  num get /* expect: avoid-unnecessary-getter */ value => _value;
}
