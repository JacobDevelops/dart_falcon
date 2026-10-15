void log(Object? value) {}

class Point {
  Point(this.x);
  final int x;
}

class Logger {
  void write(Object? value) {}
  static void shared(Object? value) {}
}

class Screen {
  final Logger logger = Logger();
  final List<int> values = [];

  void show(List<String> names) {
    names.forEach((name) => logger.write(name)); /* expect: unnecessary-lambdas */
    names.forEach((name) => describe(name)); /* expect: unnecessary-lambdas */
  }

  void describe(String name) {}
}

void main(List<String> names, List<int> numbers) {
  names.forEach((name) { /* expect: unnecessary-lambdas */
    print(name);
  });
  names.forEach((name) => log(name)); /* expect: unnecessary-lambdas */
  names.forEach((name) { /* expect: unnecessary-lambdas */
    return Logger.shared(name);
  });
  final logger = Logger();
  names.forEach((name) => logger.write(name)); /* expect: unnecessary-lambdas */
  final points = numbers.map((x) => Point(x)); /* expect: unnecessary-lambdas */
  final sink = <int>[];
  numbers.forEach((n) => sink.add(n)); /* expect: unnecessary-lambdas */
  void local(int value) {}
  numbers.forEach((n) => local(n)); /* expect: unnecessary-lambdas */
  print(points);
}

class Implicit {}

class Gen<T> {
  Gen();
}

class Queue2 {
  final List<int> items = [];
  final StringBuffer buffer = StringBuffer();

  void run(void Function() f) {}
  void named({void Function()? onTap}) {}

  void calls() {
    run(() { /* expect: unnecessary-lambdas */
      items.clear();
    });
    // A member falcon does not index on a stable receiver is a method.
    run(() => buffer.clear()); /* expect: unnecessary-lambdas */
    // A block body discarding the call in a `void`-returning named context.
    named(onTap: () { /* expect: unnecessary-lambdas */
      items.clear();
    });
  }
}

void constructors() {
  final implicit = () => Implicit(); /* expect: unnecessary-lambdas */
  final generic = () => Gen<int>(); /* expect: unnecessary-lambdas */
  final identity = () => Set<int>.identity(); /* expect: unnecessary-lambdas */
  print([implicit, generic, identity]);
}

// A `final` loop variable is stable even when its name is reused elsewhere.
void loops(List<Logger> loggers, List<String> names) {
  for (final logger in loggers) {
    names.forEach((name) => logger.write(name)); /* expect: unnecessary-lambdas */
  }
}

void reused(List<String> names) {
  var logger = Logger();
  logger = Logger();
  names.forEach(logger.write);
}

// A list literal's element type is the closure's context.
List<Point Function(int)> builders() => [(x) => Point(x)]; /* expect: unnecessary-lambdas */

void p(int value) {}

class OptionalConstructor {
  OptionalConstructor([String value = '']);
}

// Matches StringBuffer's optional Object constructor without loading SDK sources.
class Buffer {
  Buffer([Object content = '']);
}

void functionCallAndOptionalConstructors(List<int> numbers, List<String> names) {
  numbers.forEach((value) => log.call(value)); /* expect: unnecessary-lambdas */
  numbers.forEach((value) => p.call(value)); /* expect: unnecessary-lambdas */
  names.map((value) => Buffer(value)); /* expect: unnecessary-lambdas */
  names.map((value) => Exception(value)); /* expect: unnecessary-lambdas */
  names.map((value) => OptionalConstructor(value)); /* expect: unnecessary-lambdas */
  names.map((String value) => OptionalConstructor(value)); /* expect: unnecessary-lambdas */
}
