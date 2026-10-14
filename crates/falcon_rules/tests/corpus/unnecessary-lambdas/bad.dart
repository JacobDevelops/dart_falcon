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
