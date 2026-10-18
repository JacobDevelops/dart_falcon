import 'package:riverpod/riverpod.dart' as rp;
class Ref { void onDispose(void Function() callback) {} }
class Provider<T> { Provider(T Function(Ref) create); }
class Service { Service(); void dispose() {} void close() {} }
final notRiverpod = Provider((ref) => Service());
final nestedNamedFunction = rp.Provider((ref) {
  Service later() => Service();
  return later;
});
final borrowed = Service();
final prefixedBorrowed = rp.Provider((ref) => borrowed);
