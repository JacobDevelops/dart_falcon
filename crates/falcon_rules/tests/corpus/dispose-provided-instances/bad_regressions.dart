import 'package:riverpod/riverpod.dart';
class Service { Service(); void dispose() {} void close() {} }
final nestedMissing = Provider((ref) {
  if (true) {
    final service = Service(); /* expect: dispose-provided-instances */
    return service;
  }
  return null;
});
final shadowedCleanup = Provider((ref) {
  final service = Service(); /* expect: dispose-provided-instances */
  ref.onDispose(() {
    final service = Service();
    service.dispose();
  });
  return service;
});
final wrongCascade = Provider((ref) {
  final first = Service(); /* expect: dispose-provided-instances */
  final second = Service();
  ref..onDispose(second.dispose);
  return first;
});
