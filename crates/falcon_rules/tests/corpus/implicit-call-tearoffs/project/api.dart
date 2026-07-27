class Callable {
  void call() {}
}

class StaticCallable {
  static void call() {}
}

void accept(void Function() callback) {}
Callable make() => Callable();

class Provider {
  Callable create() => Callable();
}

class Sink {
  Sink(void Function() callback);
}
