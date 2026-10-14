void log(Object? value) {}

class Sink {
  void add(int value) {}
  static void record(int value) {}
}

class Processor {
  void handle(int value) {}

  void run(List<int> values) {
    for (final v in values) { /* expect: prefer-foreach */
      handle(v);
    }
  }
}

void main(List<int> values, Sink sink, void Function(int) callback) {
  for (final v in values) print(v); /* expect: prefer-foreach */
  for (var v in values) { /* expect: prefer-foreach */
    log(v);
  }
  for (final v in values) sink.add(v); /* expect: prefer-foreach */
  for (int v in values) callback(v); /* expect: prefer-foreach */
  for (final v in values) Sink.record(v); /* expect: prefer-foreach */
  for (final v in values) sink?.add(v); /* expect: prefer-foreach */
}
