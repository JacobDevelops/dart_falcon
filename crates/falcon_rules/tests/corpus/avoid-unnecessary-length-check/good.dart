class Custom {
  int get length => 2;
  void forEach(void Function(int) callback) { callback(1); }
}
void Function(int) makeCallback() { print('effect'); return print; }
List<int> get changing => <int>[];
void inspect(List<int> values, List<int> other, Iterable<int> lazy, Custom custom, dynamic unknown) {
  if (values.length > 0) print(values[0]);
  if (values.length > 1) { for (final value in values) { print(value); } }
  if (values.isNotEmpty) { for (final value in other) { print(value); } }
  if (values.isNotEmpty) { values.forEach((value) { print(value); }); print('extra'); }
  if (values.isNotEmpty) { values.forEach((value) { print(value); }); } else { print('empty'); }
  if (values.isNotEmpty) { values.forEach(makeCallback()); }
  if (lazy.length > 0) { for (final value in lazy) { print(value); } }
  if (custom.length > 0) { custom.forEach((value) { print(value); }); }
  if (unknown.length > 0) { unknown.forEach((value) { print(value); }); }
  if (changing.isNotEmpty) { for (final value in changing) { print(value); } }
}

class Holder {
  List<int> values = [];
  void inspect() {
    if (values.isNotEmpty) { values.forEach((value) { print(value); }); }
  }
}
class EffectfulHolder extends Holder {
  @override
  List<int> get values { print('read'); return []; }
  @override
  set values(List<int> value) {}
}
