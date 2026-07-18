// Good: no calls to the top-level print function.
void main() {
  debugPrint('hello');
  logger.info('message');
  final printer = Printer();
  printer.print('doc');
  stdout.write('line');
  final summary = describe();
  logger.info(summary);
}

void debugPrint(String s) {}

class Printer {
  void print(String s) {}
}

class Logger {
  void info(String s) {}
}

final logger = Logger();
final stdout = StringSink();

class StringSink {
  void write(String s) {}
}

String describe() => 'ok';
