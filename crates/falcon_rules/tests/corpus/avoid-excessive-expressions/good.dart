import 'dart:async';

void check(int value, int other, Object? item, double decimal, dynamic unknown) {
  if (value != 3 && value != 4) {}
  if (2 < value && value < 4) {}
  if (value > 2 && other > 1) {}
  if (value == 3 || value == 4) {}
  if (item != null && item is int?) {}
  if (item == null || item is! int?) {}
  if (decimal > 5 && decimal > 4) {}
  if (unknown == 3 && unknown != 4) {}
  if (value < 1 && value > 2) {}
  if (item != null && item is FutureOr<int?>) {}
  if (item == null || item is! FutureOr<dynamic>) {}
  if (item != null && item is NullableId) {}
  if (item == null || item is! NullableId) {}
}
class Example {
  int get value => 1;
  void check() { if (value > 5 && value > 4) {} }
}
extension type NullableId(int? raw) {}
