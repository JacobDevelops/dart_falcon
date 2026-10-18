import 'package:riverpod/riverpod.dart';
class Service { Service(); void dispose() {} void close() {} }
final switchMissing = Provider((ref) {
  switch (1) {
    case 1:
      final service = Service(); /* expect: dispose-provided-instances */
      return service;
    default:
      return null;
  }
});
final unrelatedNamedCleanup = Provider((ref) {
  final service = Service(); /* expect: dispose-provided-instances */
  void tearDown() {}
  ref.onDispose(tearDown);
  return service;
});
class Owned {
  Owned._();
  factory Owned() => Owned._();
  void dispose() {}
  void close() {}
}
final ownedFactory = Provider((ref) => Owned()); /* expect: dispose-provided-instances */
