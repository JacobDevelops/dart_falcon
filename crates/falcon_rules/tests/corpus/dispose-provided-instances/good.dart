import 'package:riverpod/riverpod.dart';

class Service { Service(); void dispose() {} }
class Value { Value(); }
final shared = Service();
final tearOff = Provider((ref) {
  final service = Service();
  ref.onDispose(service.dispose);
  return service;
});
final arrowCleanup = Provider((ref) {
  final service = Service();
  ref.onDispose(() => service.dispose());
  return service;
});
final blockCleanup = Provider.autoDispose((ref) {
  final service = Service();
  ref.onDispose(() { service.dispose(); });
  return service;
});
final borrowed = Provider((ref) => shared);
final immutable = Provider((ref) => Value());
final deferred = Provider((ref) => () => Service());
final twoInstances = Provider((ref) {
  final first = Service();
  final second = Service();
  ref.onDispose(first.dispose);
  ref.onDispose(() { second.dispose(); });
  return first;
});
Service outsideProvider() => Service();
void ordinaryCallback(Object Function(Ref) callback) {}
void unrelatedApi() { ordinaryCallback((ref) => Service()); }
