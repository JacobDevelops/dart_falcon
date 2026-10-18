class Ref {
  void Function() onDispose(void Function() callback) => () {};
}
class Provider<T> {
  Provider(T Function(Ref) create);
  static const autoDispose = AutoDisposeProviderBuilder();
  static const family = ProviderFamilyBuilder();
}
class AutoDisposeProviderBuilder {
  const AutoDisposeProviderBuilder();
  Provider<T> call<T>(T Function(Ref) create) => Provider(create);
  ProviderFamilyBuilder get family => const ProviderFamilyBuilder();
}
class ProviderFamilyBuilder {
  const ProviderFamilyBuilder();
  Provider<T> call<T, A>(T Function(Ref, A) create) => throw 0;
}
class FutureProvider<T> {
  FutureProvider(T Function(Ref) create);
  static const autoDispose = AutoDisposeProviderBuilder();
}
class StreamProvider<T> {
  StreamProvider(T Function(Ref) create);
  static const autoDispose = AutoDisposeProviderBuilder();
}
