import 'dart:async';

import 'types.dart';

void _futureInt(FutureOr<int>? value) {} /* expect: unnecessary-nullable */
void _futureBounded<T extends Object>(FutureOr<T>? value) {} /* expect: unnecessary-nullable */
void _extensionObject(ObjectBox? value) {} /* expect: unnecessary-nullable */
void _extensionGeneric<T extends Object>(GenericBox<T>? value) {} /* expect: unnecessary-nullable */

void call<T extends Object>(FutureOr<int> integer, FutureOr<T> bounded,
    ObjectBox box, GenericBox<T> generic) {
  _futureInt(integer);
  _futureBounded<T>(bounded);
  _extensionObject(box);
  _extensionGeneric<T>(generic);
}
