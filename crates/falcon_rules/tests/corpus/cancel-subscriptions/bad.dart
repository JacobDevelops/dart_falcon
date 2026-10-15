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
void reassigned(Stream<int> stream, StreamSubscription<int>? holder) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  holder = subscription;
  holder.cancel();
}

class SubscriptionBox {
  final StreamSubscription<int> subscription;
  SubscriptionBox(this.subscription);
}

void constructorArgument(Stream<int> stream) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  SubscriptionBox(subscription);
}

void functionArgument(Stream<int> stream, void Function(StreamSubscription<int>) callback) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  callback(subscription);
}

void assertedAlias(StreamSubscription<int>? source) {
  final subscription = source!; /* expect: cancel-subscriptions */
  subscription.pause();
}

void shadowedClosure(Stream<int> stream) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  final release = (StreamSubscription<int> subscription) => subscription.cancel();
  release(stream.listen((_) {}));
}

void localCascade(Stream<int> stream) {
  final subscription = stream.listen((_) {})..pause(); /* expect: cancel-subscriptions */
}

class ThroughThisPause {
  StreamSubscription<int>? subscription; /* expect: cancel-subscriptions */
  void hold() => this.subscription?.pause();
}

class LeakedField {
  StreamSubscription<int>? subscription; /* expect: cancel-subscriptions */
}

class ReleasedField {
  StreamSubscription<int>? subscription;
  void dispose() => subscription?.cancel();
}

class GenericSubscriptionBox<T> {
  GenericSubscriptionBox.named(StreamSubscription<T> subscription);
}

void genericConstructorArgument(Stream<int> stream) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  GenericSubscriptionBox<int>.named(subscription);
}

class FactorySubscriptionBox<T> {
  FactorySubscriptionBox._(StreamSubscription<T> subscription);

  factory FactorySubscriptionBox(Stream<T> stream) {
    final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
    return FactorySubscriptionBox<T>._(subscription);
  }
}

void dynamicCallback(Stream<int> stream, dynamic callback) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  callback(subscription);
}

void assertedCallback(Stream<int> stream, void Function(StreamSubscription<int>)? callback) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  callback!(subscription);
}

void immediateCallback(Stream<int> stream) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  ((StreamSubscription<int> value) {})(subscription);
}

void functionCallback(Stream<int> stream, Function callback) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  callback(subscription);
}

class CallbackBox {
  CallbackBox(this.callback);
  final void Function(StreamSubscription<int>) callback;

  void pass(Stream<int> stream) {
    final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
    this.callback(subscription);
  }
}

void getterCallback(Stream<int> stream, CallbackBox box) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  box.callback(subscription);
}

abstract class ChainedSubscription implements StreamSubscription<int> {
  StreamSubscription<int> next();
}

ChainedSubscription createSubscription() => throw StateError('unavailable');

void chainedCascadeRelease() {
  final subscription = createSubscription(); /* expect: cancel-subscriptions */
  subscription..next().cancel();
}

void indexedCallback(Stream<int> stream, List<void Function(StreamSubscription<int>)> callbacks) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  callbacks[0](subscription);
}

StreamSubscription<int> parenthesizedReturn(Stream<int> stream) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  return (subscription);
}

void parenthesizedCancel(Stream<int> stream) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  (subscription).cancel();
}

void parenthesizedInitializer(StreamSubscription<int> source) {
  final subscription = (source); /* expect: cancel-subscriptions */
}

class ExplicitFieldTransfer {
  StreamSubscription<int>? transferredSubscription;
  void start(Stream<int> stream) {
    final local = stream.listen((_) {}); /* expect: cancel-subscriptions */
    this.transferredSubscription = local;
  }
  void dispose() => transferredSubscription?.cancel();
}

void otherReceiverTransfer(Stream<int> stream, ReleasedField receiver) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  receiver.subscription = subscription;
}

class FieldCancelTearoff {
  late StreamSubscription<int> subscription; /* expect: cancel-subscriptions */
  void retainCancel() {
    final callback = subscription.cancel;
  }
}

class OtherSubscriptionOwner {
  StreamSubscription<int>? subscription;
  void dispose() => subscription?.cancel();
}

class SameNameIsNotSameField {
  StreamSubscription<int>? subscription; /* expect: cancel-subscriptions */
  void transfer(OtherSubscriptionOwner other, StreamSubscription<int> value) {
    other.subscription = value;
  }
}

extension type SubscriptionView(StreamSubscription<int> value) {}

class ErasedSubscriptionField {
  SubscriptionView? subscription; /* expect: cancel-subscriptions */
}

extension type SubscriptionInterface(StreamSubscription<int> value)
    implements StreamSubscription<int> {}

void extensionSubscriptionLocal(Stream<int> stream) {
  SubscriptionInterface subscription = SubscriptionInterface(stream.listen((_) {})); /* expect: cancel-subscriptions */
}
