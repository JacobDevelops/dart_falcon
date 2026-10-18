import 'package:riverpod/riverpod.dart';

class Channel { Channel(); void close() {} }
class Subscription { Subscription(); void cancel() {} }
class Service { Service(); void dispose() {} }
class Value { Value(); }
final shared = Channel();
final automatic = Provider.autoDispose((ref) => Channel());
final automaticSubscription = Provider.autoDispose((ref) => Subscription());
final registered = Provider((ref) {
  final resource = Channel();
  ref.onDispose(resource.close);
  return resource;
});
final blockCleanup = Provider((ref) {
  final resource = Subscription();
  ref.onDispose(() { resource.cancel(); });
  return resource;
});
final disposed = Provider((ref) {
  final resource = Service();
  ref.onDispose(() => resource.dispose());
  return resource;
});
final borrowed = Provider((ref) => shared);
final value = Provider((ref) => Value());
final deferred = Provider((ref) => () => Channel());
Channel outsideProvider() => Channel();
void ordinaryCallback(Object Function(Ref) callback) {}
void unrelatedApi() { ordinaryCallback((ref) => Channel()); }
