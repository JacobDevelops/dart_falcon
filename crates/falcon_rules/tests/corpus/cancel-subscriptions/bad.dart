import 'dart:async';

class Watcher {
  StreamSubscription<int>? _subscription; /* expect: cancel-subscriptions */
  late StreamSubscription<String> other; /* expect: cancel-subscriptions */

  void start(Stream<int> stream) {
    _subscription = stream.listen((_) {});
  }
}

void listenLocally(Stream<int> stream) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  subscription.pause();
}

void typedLocal(Stream<int> stream) {
  StreamSubscription<int> subscription = stream.listen(print); /* expect: cancel-subscriptions */
  subscription.resume();
}

void fromController(StreamController<int> controller) {
  var subscription = controller.stream.listen((_) {}); /* expect: cancel-subscriptions */
  print(subscription.isPaused);
}

void inClosure(Stream<int> stream) {
  final run = () {
    final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
    subscription.pause();
  };
  run();
}

void aliasedDeclaration(Stream<int> stream) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  final alias = subscription;
  alias.cancel();
}
