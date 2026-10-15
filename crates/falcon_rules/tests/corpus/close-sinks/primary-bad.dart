// @dart = 3.13

Sink<int> createSink() => throw UnimplementedError();

class DeclaringPrimary(var Sink<int> _sink) { /* expect: close-sinks */
  void initialize() {
    _sink = createSink();
  }
}
