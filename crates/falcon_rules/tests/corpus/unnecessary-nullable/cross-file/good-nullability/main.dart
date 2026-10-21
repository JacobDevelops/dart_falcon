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

class NullableOperator {
  NullableOperator? operator +(int other) => null;
  NullableOperator? operator -(int other) => null;
}

void _compound(NullableOperator? value) { value += 1; }
void _prefix(NullableOperator? value) { ++value; }
void _postfix(NullableOperator? value) { value--; }

void operatorWrites() {
  _compound(NullableOperator());
  _prefix(NullableOperator());
  _postfix(NullableOperator());
}
