import 'dart:async';
import 'package:riverpod/riverpod.dart' as rp;
class Service { Service(); void dispose() {} }
final prefixed = rp.Provider((ref) => Service()); /* expect: dispose-provided-instances */
final future = rp.FutureProvider((ref) async {
  final service = Service(); /* expect: dispose-provided-instances */
  return service;
});
final stream = rp.StreamProvider((ref) {
  final service = Service(); /* expect: dispose-provided-instances */
  return Stream.value(service);
});
final family = rp.Provider.family<Service, int>((ref, id) {
  final service = Service(); /* expect: dispose-provided-instances */
  return service;
});
