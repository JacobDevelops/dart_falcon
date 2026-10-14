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

void reassigned(Stream<int> stream, StreamSubscription<int>? holder) {
  final subscription = stream.listen((_) {});
  holder = subscription;
  holder.cancel();
}

void unresolved(dynamic stream) {
  final subscription = stream.listen((_) {});
  subscription.pause();
}
