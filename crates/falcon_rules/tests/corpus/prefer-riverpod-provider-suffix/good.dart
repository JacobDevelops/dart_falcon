import 'package:riverpod/riverpod.dart' as rp;
final countProvider = rp.Provider<int>((ref) => 0);
final futureProvider = rp.FutureProvider<int>((ref) => 0);
final streamProvider = rp.StreamProvider<int>((ref) => 0);
final familyProvider = rp.ProviderFamily<int, int>((ref, arg) => arg);
final autoProvider = rp.AutoDisposeProvider<int>((ref) => 0);
class Providers {
  static final countProvider = rp.Provider<int>((ref) => 0);
  final instance = rp.Provider<int>((ref) => 0);
}
void localValues() { final local = rp.Provider<int>((ref) => 0); }
class Provider<T> { Provider(T Function(Object) create); }
final unrelated = Provider<int>((ref) => 0);
final ordinary = 0;
