import 'dart:async';
import 'dart:io';

class Logger {
  IOSink? _sink; /* expect: close-sinks */

  void open(File file) {
    _sink = file.openWrite();
  }
}

void writeLocally(File file) {
  final sink = file.openWrite(); /* expect: close-sinks */
  sink.write('entry');
}

void controllerLocal() {
  final controller = StreamController<int>(); /* expect: close-sinks */
  controller.add(1);
}

void typedLocal(File file) {
  IOSink sink = file.openWrite(); /* expect: close-sinks */
  sink.writeln('entry');
}

void socketLocal(Socket socket) {
  Socket connection = socket;
  Socket other = getSocket(); /* expect: close-sinks */
  other.write('x');
  connection.write('y');
}

Socket getSocket() => throw UnimplementedError();
