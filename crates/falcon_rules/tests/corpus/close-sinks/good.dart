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

void closedFromClosure() {
  final sink = StreamController<int>();
  final dispose = () => sink.close();
  dispose();
}

void releaseLaterSection(File file) {
  final sink = file.openWrite();
  sink
    ..toString()
    ..close();
}

abstract class ResourceBase {
  final events = StreamController<int>();
}

class ResourceChild extends ResourceBase {
  void dispose() => events.close();
}

abstract class ThisResourceBase {
  final events = StreamController<int>();
}

class ThisResourceChild extends ThisResourceBase {
  void dispose() => this.events.close();
}

abstract class MixinResourceBase {
  final events = StreamController<int>();
}

mixin ResourceReleaser on MixinResourceBase {
  void dispose() => events.close();
}

abstract class ExtensionResourceBase {
  final events = StreamController<int>();
}

extension ResourceRelease on ExtensionResourceBase {
  void dispose() => events.close();
}

void siblingCases(int value) {
  switch (value) {
    case 1:
      final sink = StreamController<int>();
      sink.close();
    case 2:
      final sink = StreamController<int>();
      sink.close();
  }
}

void collectionScope() {
  final values = [for (var sink = 0; sink < 1; sink++) sink];
  final sink = StreamController<int>();
  sink.close();
}

void mapCollectionScope() {
  final values = {for (var sink = 0; sink < 1; sink++) sink: sink};
  final sink = StreamController<int>();
  sink.close();
}

void nestedFormal(File file, void callback(IOSink sink)) {
  final sink = file.openWrite();
  sink.close();
}

class ChainedReceiver {
  ChainedReceiver next() => this;
  void take(Sink<int> sink) {}
}

void chainedArgument(ChainedReceiver receiver) {
  final sink = StreamController<int>();
  receiver..next().take(sink);
}

final log = Object();

class MethodShadowsVariable {
  void log(Sink<int> sink) {}
  void write() {
    final sink = StreamController<int>();
    log(sink);
  }
}

void localFunctionShadowsVariable() {
  final take = Object();
  {
    void take(Sink<int> sink) {}
    final sink = StreamController<int>();
    take(sink);
  }
}

abstract class ThisExtensionResourceBase {
  final events = StreamController<int>();
}

extension ThisResourceRelease on ThisExtensionResourceBase {
  void dispose() => this.events.close();
}

class LocalFunctionShadowsField {
  LocalFunctionShadowsField(this.report);
  final void Function(Object) report;

  void run() {
    void report(Sink<int> value) {}
    final sink = StreamController<int>();
    report(sink);
  }
}
