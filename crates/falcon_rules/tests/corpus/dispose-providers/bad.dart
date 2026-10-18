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

final other = Disposable();
Disposable makeDisposable(dynamic context) => Disposable();

void examples() {
  p.Provider(create: (_) => Disposable()); /* expect: dispose-providers */
  p.Provider(create: (_) => Closable()); /* expect: dispose-providers */
  p.Provider(create: (_) => Cancelable()); /* expect: dispose-providers */
  p.Provider(create: makeDisposable); /* expect: dispose-providers */
  p.Provider(create: (_) { return Disposable(); }); /* expect: dispose-providers */
  new p.Provider(create: (_) => InheritedDisposable()); /* expect: dispose-providers */
  p.Provider(create: (_) => Disposable(), dispose: (_, value) {}); /* expect: dispose-providers */
  p.Provider(create: (_) => Disposable(), dispose: (_, value) => other.dispose()); /* expect: dispose-providers */
  p.Provider(create: (_) => Disposable(), dispose: (_, value) => value.toString()); /* expect: dispose-providers */
  p.Provider(create: (_) => Disposable(), dispose: (_, value) { /* expect: dispose-providers */
    void later() { value.dispose(); }
  });
  p.Provider(create: (_) => Disposable(), dispose: (_, value) { /* expect: dispose-providers */
    { final value = other; value.dispose(); }
  });
}
