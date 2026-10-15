import 'dart:convert';

void log(Object? value) {}

void pair(Object? a, Object? b) {}

class Logger {
  Logger();
  const Logger.constant();
  void write(Object? value) {}
}

class Screen {
  Logger logger = Logger();

  void show(List<String> names) {
    names.forEach((name) => logger.write(name));
    names.forEach((name) => this.write(name));
  }

  void write(String name) {}
}

Logger currentLogger() => Logger();

void main(List<String> names, List<List<String>> groups) async {
  names.forEach((name) => log(name.length));
  names.forEach((name) => pair(name, name));
  names.forEach((name) {
    log(name);
    log(name);
  });
  names.forEach((name) async => log(name));
  names.forEach((name) => currentLogger().write(name));
  var logger = Logger();
  names.forEach((name) => logger.write(name));
  Logger? maybe = Logger();
  names.forEach((name) => maybe?.write(name));
  groups.forEach((group) => group.add('x'));
  final callback = (name) => logger.write(name);
  names.forEach(([Object? name]) => log(name));
  print(callback);
}

class Box {
  Box(this.name);
  final String name;
  static const Logger shared = Logger.constant();
}

enum Level {
  info;

  void write(Object? value) {}
}

const topLogger = Logger.constant();

class Recorder {
  List<int> states = [];

  Recorder(Stream<int> stream) {
    // A non-final field is not a stable receiver.
    stream.listen((state) {
      states.add(state);
    });
  }
}

void contexts(List<String> names) {
  // The closure's own type returns `Null`, which `log`'s tear-off does not fit.
  final f = (String name) {
    log(name);
  };
  // The analyzer does not count `const` variables and fields as final.
  names.forEach((name) => Box.shared.write(name));
  names.forEach((name) => topLogger.write(name));
  const local = Logger.constant();
  names.forEach((name) => local.write(name));
  names.forEach((name) => Level.info.write(name));
  // A block body is not checked for constructor calls.
  names.map((name) {
    return Box(name);
  });
  // A constructor tear-off must fit the closure's (dynamic) parameter type.
  final g = (name) => Box(name);
  print([f, g]);
}

class Lazy {
  late final Logger logger = Logger();

  // A `late` field is not stable.
  void show(List<String> names) {
    names.forEach((name) => logger.write(name));
  }
}

// Imported SDK const values are not stable final receivers.
Future<dynamic> decodeInput(Future<String> input) =>
    input.then((response) => json.decode(response));
