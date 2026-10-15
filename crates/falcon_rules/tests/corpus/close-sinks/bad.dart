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
class CascadingBloc {
  final StreamController<int> cascadingEvents = StreamController<int>(); /* expect: close-sinks */

  void addTwice(int event) {
    this.cascadingEvents
      ..add(event)
      ..add(event);
  }
}

class SinkBox {
  final Sink<int> sink;
  SinkBox(this.sink);
  SinkBox.named(this.sink);
}

void constructorArgument() {
  final sink = StreamController<int>(); /* expect: close-sinks */
  SinkBox(sink);
}

void namedConstructorArgument() {
  final sink = StreamController<int>(); /* expect: close-sinks */
  SinkBox.named(sink);
}

void localCascade() {
  final sink = StreamController<int>()..add(1); /* expect: close-sinks */
}

void sameNamedBlocks() {
  {
    final sink = StreamController<int>(); /* expect: close-sinks */
    sink.add(1);
  }
  {
    final sink = StreamController<int>();
    sink.close();
  }
}

class LeakedField {
  final sink = StreamController<int>(); /* expect: close-sinks */
}

class ReleasedField {
  final sink = StreamController<int>();
  void dispose() => sink.close();
}

class PassedField {
  final sink = StreamController<int>(); /* expect: close-sinks */
  void pass() => takeSink(sink);
}

void takeSink(Sink<int> value) {}

void functionArgument(void Function(Sink<int>) callback) {
  final sink = StreamController<int>(); /* expect: close-sinks */
  callback(sink);
}

class GenericSinkBox<T> {
  GenericSinkBox.named(Sink<T> sink);
}

void genericConstructorArgument() {
  final sink = StreamController<int>(); /* expect: close-sinks */
  GenericSinkBox<int>.named(sink);
}

class FactorySinkBox<T> {
  FactorySinkBox._(Sink<T> sink);

  factory FactorySinkBox() {
    final StreamController<T> sink = StreamController<T>.broadcast(); /* expect: close-sinks */
    return FactorySinkBox<T>._(sink);
  }
}

void dynamicCallback(dynamic callback) {
  final sink = StreamController<int>(); /* expect: close-sinks */
  callback(sink);
}

void functionCallback(Function callback) {
  final sink = StreamController<int>(); /* expect: close-sinks */
  callback(sink);
}

void assertedCallback(void Function(Sink<int>)? callback) {
  final sink = StreamController<int>(); /* expect: close-sinks */
  callback!(sink);
}

void immediateCallback() {
  final sink = StreamController<int>(); /* expect: close-sinks */
  ((Sink<int> value) {})(sink);
}

class CallbackBox {
  CallbackBox(this.callback);
  final void Function(Sink<int>) callback;

  void pass() {
    final sink = StreamController<int>(); /* expect: close-sinks */
    this.callback(sink);
  }
}

void getterCallback(CallbackBox box) {
  final sink = StreamController<int>(); /* expect: close-sinks */
  box.callback(sink);
}

class ChainedSink implements Sink<int> {
  void add(int value) {}
  void close() {}
  Sink<int> next() => this;
}

void chainedCascadeRelease() {
  final sink = ChainedSink(); /* expect: close-sinks */
  sink..next().close();
}

void indexedCallback(List<void Function(Sink<int>)> callbacks) {
  final sink = StreamController<int>(); /* expect: close-sinks */
  callbacks[0](sink);
}
