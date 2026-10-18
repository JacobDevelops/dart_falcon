import 'package:riverpod/riverpod.dart';
class Service { Service(); void dispose() {} void close() {} }
final storedCleanup = Provider((ref) {
  final service = Service();
  final tearDown = () => service.dispose();
  ref.onDispose(tearDown);
  return service;
});
final storedBlockCleanup = Provider((ref) {
  final service = Service();
  final tearDown = () { service.dispose(); };
  ref.onDispose(tearDown);
  return service;
});
