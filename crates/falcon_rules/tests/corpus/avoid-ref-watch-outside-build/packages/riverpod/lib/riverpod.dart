class ProviderListenable<T> {}
class Provider<T> extends ProviderListenable<T> {
  Provider(T Function(Ref) create);
}
abstract class Ref {
  T watch<T>(ProviderListenable<T> provider);
  T read<T>(ProviderListenable<T> provider);
}
abstract class Notifier<T> {}
abstract class AsyncNotifier<T> {}
