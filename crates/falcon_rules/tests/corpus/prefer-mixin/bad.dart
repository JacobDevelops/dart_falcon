class Logger {
  void log(String message) {}
}

class Tracer {
  void trace() {}
}

abstract class Disposable {
  void dispose() {}
}

class Service with Logger {} /* expect: prefer-mixin */

class Worker extends Object with Logger, Tracer {} /* expect: prefer-mixin */ /* expect: prefer-mixin */

class Resource with Disposable {} /* expect: prefer-mixin */

class Alias = Object with Tracer; /* expect: prefer-mixin */

enum Level with Logger { /* expect: prefer-mixin */
  low,
  high,
}

class LocalLoggerConsumer with Logger {} /* expect: prefer-mixin */
