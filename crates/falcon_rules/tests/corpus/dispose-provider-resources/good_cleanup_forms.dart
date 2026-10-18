import 'package:riverpod/riverpod.dart';
class Service { Service(); void dispose() {} void close() {} }
void disposeAll(List<Service> services) { for (final service in services) { service.dispose(); } }
final namedCleanup = Provider((ref) {
  final service = Service();
  void tearDown() { service.dispose(); }
  ref.onDispose(tearDown);
  return service;
});
final delegatedCleanup = Provider((ref) {
  final service = Service();
  ref.onDispose(() => disposeAll([service]));
  return service;
});
final storedRegistration = Provider((ref) {
  final service = Service();
  final remove = ref.onDispose(service.dispose);
  return service;
});
final switchCleanup = Provider((ref) {
  switch (1) {
    case 1:
      final service = Service();
      ref.onDispose(service.dispose);
      return service;
    default:
      return null;
  }
});
class Singleton {
  Singleton._();
  static final shared = Singleton._();
  factory Singleton() => shared;
  void dispose() {}
  void close() {}
}
final borrowedFactory = Provider((ref) => Singleton());
