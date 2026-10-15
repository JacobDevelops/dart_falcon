// @dart = 3.13

class InjectedPrimary(Sink<int> sink) {
  final Sink<int> _sink;
  this : this._sink = sink;
}

extension type SinkRepresentation(Sink<int> sink) {}
