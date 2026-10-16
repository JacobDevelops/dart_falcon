import 'dart:async';

class Lock {
  Future<T> shared<T>(Future<T> Function() action) => action();
  Future<U> exclusive<U>(Future<U> Function() action) => action();
}

Future<T> locked<T>(Lock lock, bool shared, Future<T> Function() action) {
  final block = shared ? lock.shared : lock.exclusive;
  return block(() async {
    return await action(); /* expect: unnecessary-await-in-return */
  });
}
