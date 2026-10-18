import 'package:riverpod/riverpod.dart';
class Service {
  Service();
  void dispose() {}
  void close() {}
  static final shared = Service();
  static Service getInstance() => shared;
}
final sharedFactory = Provider((ref) => Service.getInstance());
final nestedCleanup = Provider((ref) {
  if (true) {
    final service = Service();
    ref.onDispose(service.dispose);
    return service;
  }
  return Service.shared;
});
final cascadedCleanup = Provider((ref) {
  final first = Service();
  final second = Service();
  ref..onDispose(first.dispose)..onDispose(() => second.dispose());
  return first;
});
