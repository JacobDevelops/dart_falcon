import 'dart:async';

import 'types.dart';

void _futureNullable(FutureOr<int?>? value) {}
void _futureUnbounded<T>(FutureOr<T>? value) {}
void _extensionNullable(NullableBox? value) {}
void _extensionUnbounded<T>(UnboundedBox<T>? value) {}
void _extensionWithoutObject(IntBox? value) {}

void call<T>(FutureOr<int?> nullable, FutureOr<T> unbounded,
    NullableBox box, UnboundedBox<T> generic, IntBox integer) {
  _futureNullable(nullable);
  _futureUnbounded<T>(unbounded);
  _extensionNullable(box);
  _extensionUnbounded<T>(generic);
  _extensionWithoutObject(integer);
}
