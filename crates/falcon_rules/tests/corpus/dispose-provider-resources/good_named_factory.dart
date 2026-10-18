import 'package:riverpod/riverpod.dart' as rp;
class Service { Service(); void dispose() {} void close() {} }
final deferredNamed = rp.Provider((ref) {
  Service later() { return Service(); }
  return later;
});
