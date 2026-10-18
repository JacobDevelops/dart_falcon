import 'package:provider/provider.dart' as p;

class Disposable {
  void dispose() {}
}

class Closable {
  void close() {}
}

class Cancelable {
  void cancel() {}
}

class InheritedDisposable extends Disposable {}
class Plain {}
Disposable makeDisposable(dynamic context) => Disposable();

class StaticCleanup { static void dispose() {} }

final shared = Disposable();
T getIt<T>() => shared as T;
class Api extends Disposable {
  Api();
  factory Api.cached() => singleton;
  static final singleton = Api();
  static Api instance() => singleton;
}

// A local constructor with the same spelling is not a package provider.
class Provider {
  Provider({required Object Function(Object?) create});
}

void examples() {
  p.Provider(create: (_) => Disposable(), dispose: (_, value) => value.dispose());
  p.Provider(create: (_) => Closable(), dispose: (_, value) { value.close(); });
  p.Provider(create: (_) => Cancelable(), dispose: (_, value) => value.cancel());
  new p.Provider(create: (_) => InheritedDisposable(), dispose: (_, value) => value.dispose());
  p.Provider(create: (_) => Disposable(), dispose: (_, value) => value..dispose());
  p.Provider.value(value: Disposable());
  p.Provider(create: makeDisposable, dispose: (_, value) => value.dispose());
  p.Provider(create: (_) { return Disposable(); }, dispose: (_, value) { value.dispose(); });
  p.Provider(create: (_) => Plain());
  p.Provider(create: (_) => StaticCleanup());
  Provider(create: (_) => Disposable());
  p.Provider(create: (_) => Disposable(), dispose: (_, value) => value!.dispose());
  p.Provider(create: (_) => Disposable(), dispose: (_, value) => (value as Disposable).dispose());
  final existing = Disposable();
  p.Provider(create: (_) => existing);
  p.Provider(create: (dynamic context) => context.read<Disposable>());
  p.Provider(create: (_) => getIt<Disposable>());
  p.Provider(create: (_) => Api.instance());
  p.Provider(create: (_) => Api.cached());
}
