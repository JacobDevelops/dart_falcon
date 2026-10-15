import 'dart:async';

void consume(StreamSubscription<int> subscription) {}

class Watcher {
  StreamSubscription<int>? _subscription;

  void start(Stream<int> stream) {
    _subscription = stream.listen((_) {});
  }

  Future<void> dispose() async {
    await _subscription?.cancel();
  }
}

class NullAsserted {
  StreamSubscription<int>? _subscription;

  void dispose() {
    _subscription!.cancel();
  }
}

class ThroughThis {
  StreamSubscription<int>? subscription;

  void dispose() {
    this.subscription?.cancel();
  }
}

class Paused {
  StreamSubscription<int>? _pausedSubscription;

  void start(Stream<int> stream) {
    _pausedSubscription = stream.listen((_) {});
  }

  void hold() {
    _pausedSubscription?.pause();
  }
}

class Injected {
  final StreamSubscription<int> subscription;

  Injected(this.subscription);
}

class Initialized {
  final StreamSubscription<int> subscription;

  Initialized(Stream<int> stream) : subscription = stream.listen((_) {});
}

void cancelled(Stream<int> stream) {
  final subscription = stream.listen((_) {});
  subscription.cancel();
}

StreamSubscription<int> returned(Stream<int> stream) {
  final subscription = stream.listen((_) {});
  return subscription;
}

void passed(Stream<int> stream) {
  final subscription = stream.listen((_) {});
  consume(subscription);
}

void tornOff(Stream<int> stream, void Function(Future<void> Function()) register) {
  final subscription = stream.listen((_) {});
  register(subscription.cancel);
}

void cascaded(Stream<int> stream) {
  final subscription = stream.listen((_) {});
  subscription
    ..pause()
    ..cancel();
}

void unresolved(dynamic stream) {
  final subscription = stream.listen((_) {});
  subscription.pause();
}

void cancelledFromClosure(Stream<int> stream) {
  final subscription = stream.listen((_) {});
  final dispose = () => subscription.cancel();
  dispose();
}

class CascadingField {
  StreamSubscription<int>? subscription;
  void hold() {
    subscription?..pause();
  }
}

abstract class ResourceBase {
  StreamSubscription<int>? subscription;
}

class ResourceChild extends ResourceBase {
  void dispose() => subscription?.cancel();
}

abstract class ThisResourceBase {
  StreamSubscription<int>? subscription;
}

class ThisResourceChild extends ThisResourceBase {
  void dispose() => this.subscription?.cancel();
}

void siblingCases(Stream<int> stream, int value) {
  switch (value) {
    case 1:
      final subscription = stream.listen((_) {});
      subscription.cancel();
    case 2:
      final subscription = stream.listen((_) {});
      subscription.cancel();
  }
}

void collectionScope(Stream<int> stream) {
  final values = [for (var subscription = 0; subscription < 1; subscription++) subscription];
  final subscription = stream.listen((_) {});
  subscription.cancel();
}

void nestedFormal(Stream<int> stream, void callback(StreamSubscription<int> subscription)) {
  final subscription = stream.listen((_) {});
  subscription.cancel();
}
