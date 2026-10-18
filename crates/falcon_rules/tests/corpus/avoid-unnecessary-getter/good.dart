class Example {
  final int value = 1;
  int _mutable = 1;
  int get mutable => _mutable;
  int get computed => value + 1;
}
class Pair {
  int _value = 1;
  int get value => _value;
  set value(int value) { _value = value; }
}
class Annotated {
  final int _value = 1;
  @override
  int get hashCode => _value + 1;
}
class Upcast {
  final int _value = 1;
  num get value => _value;
}
class Private {
  final int _value = 1;
  int get _alias => _value;
}
class Late {
  late final int _value;
  int get value => _value;
}
