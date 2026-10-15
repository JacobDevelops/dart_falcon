import 'dart:async';
import 'dart:collection';

class Holder {
  static StreamSubscription<int>? subscription;
  static void dispose() => subscription?.cancel();
}

class Receiver {
  StreamSubscription<int>? subscription;
  void dispose() => subscription?.cancel();
}

void staticWrite(Stream<int> stream) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  Holder.subscription = subscription;
}

void nullAwareWrite(Stream<int> stream, Receiver? receiver) {
  final subscription = stream.listen((_) {});
  receiver?.subscription = subscription;
}

void dynamicIndexWrite(Stream<int> stream, dynamic dynamicList) {
  final subscription = stream.listen((_) {});
  dynamicList[0] = subscription;
}

void typedIndexWrite(Stream<int> stream, List<StreamSubscription<int>> typedList) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  typedList[0] = subscription;
}

void cascadePropertyWrite(Stream<int> stream, Receiver receiver) {
  final subscription = stream.listen((_) {});
  receiver..subscription = subscription;
}

void dynamicCascadeWrite(Stream<int> stream, dynamic receiver) {
  final subscription = stream.listen((_) {});
  receiver..unrelated = subscription;
}

void dynamicCascadeIndexWrite(Stream<int> stream, dynamic receiver, int index) {
  final subscription = stream.listen((_) {});
  receiver..[index] = subscription;
}

void typedCascadeIndexWrite(
    Stream<int> stream, List<StreamSubscription<int>> receiver, int index) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  receiver..[index] = subscription;
}

void typedMapWrite(
    Stream<int> stream, Map<int, StreamSubscription<int>> receiver) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  receiver[0] = subscription;
}

void sdkMapWrite(
    Stream<int> stream, HashMap<int, StreamSubscription<int>> receiver) {
  final subscription = stream.listen((_) {}); /* expect: cancel-subscriptions */
  receiver[0] = subscription;
}

// Dynamic remains dynamic through every selector in a cascade chain.
void dynamicCascadeFieldChain(Stream<int> stream, dynamic receiver) {
  final sub = stream.listen((_) {});
  receiver..inner.slot = sub;
}

void dynamicCascadeCallChain(Stream<int> stream, dynamic receiver) {
  final sub = stream.listen((_) {});
  receiver..lookup()[0] = sub;
}

void dynamicCascadeIndexChain(Stream<int> stream, dynamic receiver) {
  final sub = stream.listen((_) {});
  receiver..[0].slot = sub;
}

class TypedChain {
  TypedChain get inner => this;
  TypedChain lookup() => this;
  TypedChain operator [](int index) => this;
  set slot(StreamSubscription<int> value) {}
  void operator []=(int index, StreamSubscription<int> value) {}
}

void typedCascadeChains(Stream<int> stream, TypedChain receiver) {
  final fieldSub = stream.listen((_) {}); /* expect: cancel-subscriptions */
  receiver..inner.slot = fieldSub;
  final callSub = stream.listen((_) {}); /* expect: cancel-subscriptions */
  receiver..lookup()[0] = callSub;
  final indexSub = stream.listen((_) {}); /* expect: cancel-subscriptions */
  receiver..[0].slot = indexSub;
}

// Static writes suppress only the exact field declaration, not a local resource.
class InsideStaticOwner {
  static StreamSubscription<int>? inside;
  static void adopt(StreamSubscription<int> value) {
    InsideStaticOwner.inside = value;
  }
}

class OutsideStaticOwner {
  static StreamSubscription<int>? outside;
}

void adoptOutside(StreamSubscription<int> value) {
  OutsideStaticOwner.outside = value;
}

class UnwrittenStaticOwner {
  static StreamSubscription<int>? outside; /* expect: cancel-subscriptions */
}

void register<T>(StreamSubscription<int> value) {}

void nestedGenericArguments(Stream<int> stream) {
  final doubleClose = stream.listen((_) {});
  register<List<int>>(doubleClose);
  final tripleClose = stream.listen((_) {});
  register<List<List<int>>>(tripleClose);
  final grouped = stream.listen((_) {}); /* expect: cancel-subscriptions */
  register<List<int>>((grouped));
}
