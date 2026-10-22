import 'dart:async' as async;
import 'dart:core' as core;
import 'package:flutter/foundation.dart' as flutter;

typedef FutureAlias<T> = async.Future<T>;

void prefixedFuture() {
  try {} catch (error) {
    async.Future<void>.error(error);
  }
}

void explicitFuture() {
  try {} catch (error) {
    new async.Future<void>.error(error);
  }
}

void aliasedFuture() {
  try {} catch (error) {
    FutureAlias<void>.error(error);
  }
}

void flutterReport() {
  try {} catch (error) {
    flutter.FlutterError.reportError(error);
  }
}

void coreThrow() {
  try {} catch (error, stack) {
    core.Error.throwWithStackTrace(error, stack);
  }
}
