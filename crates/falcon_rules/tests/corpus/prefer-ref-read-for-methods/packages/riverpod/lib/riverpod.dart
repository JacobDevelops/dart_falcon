export 'package:state_notifier/state_notifier.dart';
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
abstract class NotifierBase<T> {}
abstract class AutoDisposeNotifier<T> extends NotifierBase<T> {}
abstract class StreamNotifier<T> {}
class Refreshable<T> implements ProviderListenable<T> {}
class NotifierProvider<N extends Notifier<T>, T> extends ProviderListenable<T> {
  NotifierProvider(N Function() create);
  Refreshable<N> get notifier => Refreshable<N>();
}
