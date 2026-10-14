import 'dart:async';
import 'dart:io';

void consume(IOSink sink) {}

class Logger {
  IOSink? _sink;

  void open(File file) {
    _sink = file.openWrite();
  }

  Future<void> dispose() async {
    await _sink?.close();
  }
}

class Pipe {
  final StreamController<String> controller = StreamController<String>();

  void dispose() => controller.close();
}

class SendingPipe {
  final StreamController<String> sendingController = StreamController<String>();

  void send(String value) => sendingController.add(value);
}

class Bloc {
  final StreamController<int> _events = StreamController<int>();

  void add(int event) => _events.add(event);
}

class NullableBloc {
  StreamController<int>? nullableEvents;

  void add(int event) => nullableEvents?.add(event);
}

class CascadingBloc {
  final StreamController<int> cascadingEvents = StreamController<int>();

  void addTwice(int event) {
    this.cascadingEvents
      ..add(event)
      ..add(event);
  }
}

class Injected {
  final IOSink sink;

  Injected(this.sink);
}

Future<void> closed(File file) async {
  final sink = file.openWrite();
  sink.write('entry');
  await sink.close();
}

IOSink returned(File file) {
  final sink = file.openWrite();
  return sink;
}

void passed(File file) {
  final sink = file.openWrite();
  consume(sink);
}

void cascaded(File file) {
  final sink = file.openWrite();
  sink
    ..write('a')
    ..close();
}

void destroyed(Socket socket) {
  final Socket connection = socketFor(socket);
  connection.destroy();
}

Socket socketFor(Socket socket) => socket;

void unresolved(dynamic file) {
  final sink = file.openWrite();
  sink.write('entry');
}
