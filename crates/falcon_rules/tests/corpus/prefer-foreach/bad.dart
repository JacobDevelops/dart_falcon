void log(Object? value) {}

class Sink {
  void add(int value) {}
  static void record(int value) {}
}

class Base {
  final Sink sink = Sink();
}

class Processor extends Base {
  void handle(int value) {}

  // An inherited field as the receiver.
  void inherited(List<int> values) {
    for (final v in values) { /* expect: prefer-foreach */
      sink.add(v);
    }
  }

  void run(List<int> values) {
    for (final v in values) { /* expect: prefer-foreach */
      handle(v);
    }
  }
}

void examples(List<int> values, Sink sink, void Function(int) callback) {
  for (final v in values) print(v); /* expect: prefer-foreach */
  for (var v in values) { /* expect: prefer-foreach */
    log(v);
  }
  for (final v in values) sink.add(v); /* expect: prefer-foreach */
  for (int v in values) callback(v); /* expect: prefer-foreach */
  for (final v in values) Sink.record(v); /* expect: prefer-foreach */
  for (final v in values) sink?.add(v); /* expect: prefer-foreach */
}
