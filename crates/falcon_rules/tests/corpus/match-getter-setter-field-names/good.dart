class State {
  int _count = 0;
  int get count => _count;
  set count(int value) => _count = value;
  int get doubled => _count * 2;
  set positive(int value) { if (value > 0) _count = value; }
  int get constant => 42;
  int get externalValue => unknown;
  set checked(int value) { print(value); _count = value; }
  set parameter(int _count) => _count = _count;
}
class Other {
  int _size = 0;
  int get size { return this._size; }
}
int unknown = 0;
class Aliases {
  bool _enabled = false;
  bool get isEnabled => _enabled;
  int _count = 0;
  int get size => _count;
  int get length => _count;
  int __value = 0;
  int get value => __value;
}
abstract class Contract {
  int get value;
}
class Inherited implements Contract {
  int _count = 0;
  int _value = 1;
  int get value => _count;
}
