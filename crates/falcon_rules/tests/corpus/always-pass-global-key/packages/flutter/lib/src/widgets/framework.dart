class Key { const Key(); }
class GlobalKey<T> extends Key {
  GlobalKey();
  GlobalKey.debug({String? debugLabel});
  Object? get currentState => null;
  Object? get currentContext => null;
}
class LabeledGlobalKey<T> extends GlobalKey<T> { LabeledGlobalKey(String? label); }
class GlobalObjectKey<T> extends GlobalKey<T> { GlobalObjectKey(Object value); }
class ValueKey<T> extends Key { const ValueKey(T value); }
class Widget { const Widget({Key? key}); }
abstract class State<T extends Widget> {}
