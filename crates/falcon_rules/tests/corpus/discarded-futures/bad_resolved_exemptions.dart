import 'dart:async' as real;

class Future {
  static real.Future<void> delayed(Duration d, void Function() computation) async {}
}

abstract class PendingMap implements Map<String, real.Future<void>> {
  real.Future<void> putIfAbsent(String key, real.Future<void> Function() ifAbsent);
}

void sync(PendingMap map) {
  Future.delayed(Duration.zero, () {}); /* expect: discarded-futures */
  map.putIfAbsent('a', () => real.Future<void>.value()); /* expect: discarded-futures */
}
