import 'dart:async';

// Qualified static writes retain their declaring field's identity.
class InsideSinkOwner {
  static Sink<int>? inside;
  static void adopt(Sink<int> value) {
    InsideSinkOwner.inside = value;
  }
}

class OutsideSinkOwner {
  static Sink<int>? outside;
}

void adoptOutside(Sink<int> value) {
  OutsideSinkOwner.outside = value;
}

class UnwrittenSinkOwner {
  static Sink<int>? outside; /* expect: close-sinks */
}

void dynamicCascadeChains(dynamic receiver) {
  final fieldSink = StreamController<int>();
  receiver..inner.slot = fieldSink;
  final callSink = StreamController<int>();
  receiver..lookup()[0] = callSink;
  final indexSink = StreamController<int>();
  receiver..[0].slot = indexSink;
}

void register<T>(Sink<int> value) {}

void nestedGenericArguments() {
  final doubleClose = StreamController<int>();
  register<List<int>>(doubleClose);
  final tripleClose = StreamController<int>();
  register<List<List<int>>>(tripleClose);
  final grouped = StreamController<int>(); /* expect: close-sinks */
  register<List<int>>((grouped));
}
