// `late` on already-lazy static / top-level variables with initializers.
late String banner = 'hello'; /* expect: unnecessary-late */
late int total = compute(); /* expect: unnecessary-late */

// `late final` with an initializer is equally redundant: static / top-level
// finals are already lazily initialized, so `late` still adds nothing.
late final int cachedTotal = compute(); /* expect: unnecessary-late */

int compute() => 42;

class Service {
  static late Service instance = Service._(); /* expect: unnecessary-late */
  static late int counter = 0; /* expect: unnecessary-late */
  static late final int cached = 0; /* expect: unnecessary-late */
  Service._();
}

class Cache {
  static late String key = 'k'; /* expect: unnecessary-late */
  static late bool ready = true; /* expect: unnecessary-late */
}
