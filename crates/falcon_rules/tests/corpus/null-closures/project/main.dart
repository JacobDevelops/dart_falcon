import 'dart:async' as async;
import 'helper.dart';

void check(NullApi local, List<int> values) {
  async.Future.microtask(null); /* expect: null-closures */
  local.any(null);
  values.map(null);
}
