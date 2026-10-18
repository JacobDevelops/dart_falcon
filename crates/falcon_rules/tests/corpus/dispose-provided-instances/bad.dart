import 'package:riverpod/riverpod.dart';

class Service {
  Service();
  Service.named();
  void dispose() {}
}
class Child extends Service {}

final plain = Provider((ref) {
  final service = Service(); /* expect: dispose-provided-instances */
  return service;
});
final automatic = Provider.autoDispose((ref) {
  final service = Service(); /* expect: dispose-provided-instances */
  return service;
});
final arrow = Provider((ref) => Service()); /* expect: dispose-provided-instances */
final explicitNew = Provider((ref) {
  final service = new Service(); /* expect: dispose-provided-instances */
  return service;
});
final named = Provider((ref) {
  final service = Service.named(); /* expect: dispose-provided-instances */
  return service;
});
final inherited = Provider((ref) {
  final service = Child(); /* expect: dispose-provided-instances */
  return service;
});
final wrongInstance = Provider((ref) {
  final owned = Service(); /* expect: dispose-provided-instances */
  ref.onDispose(() {});
  return owned;
});
final mereMention = Provider((ref) {
  final service = Service(); /* expect: dispose-provided-instances */
  ref.onDispose(() { service.dispose; });
  return service;
});
final deferred = Provider((ref) {
  final service = Service(); /* expect: dispose-provided-instances */
  ref.onDispose(() { final later = () => service.dispose(); });
  return service;
});
