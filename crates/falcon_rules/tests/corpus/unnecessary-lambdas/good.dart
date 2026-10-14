void log(Object? value) {}

void pair(Object? a, Object? b) {}

class Logger {
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
  names.forEach(([name]) => log(name));
  print(callback);
}
