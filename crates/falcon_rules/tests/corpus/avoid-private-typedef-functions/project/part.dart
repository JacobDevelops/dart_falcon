part of 'owner.dart';

void invokeOnce(_UsedOnce callback) => callback();

void invokeTwice(_Reused first, _Reused second) {
  first();
  second();
}
