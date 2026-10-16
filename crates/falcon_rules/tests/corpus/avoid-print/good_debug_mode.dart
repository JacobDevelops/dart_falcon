import 'package:flutter/foundation.dart';

void blockGuard(bool other) {
  if (kDebugMode) {
    print('debug');
    if (other) print('nested');
  }
}

void singleGuard() {
  if (kDebugMode) print('debug');
}

void nestedFunctionGuard() {
  void local() {
    if (kDebugMode) print('local debug');
  }
  local();
}
