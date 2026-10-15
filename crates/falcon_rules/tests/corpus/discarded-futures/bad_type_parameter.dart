abstract class Controller {
  Future<void> dispose();
}

void discard<T extends Controller>(T controller, T? optional) {
  controller.dispose(); /* expect: discarded-futures */
  optional?.dispose(); /* expect: discarded-futures */
}

abstract class View<T extends Controller> {
  T get controller;

  void close() {
    controller.dispose(); /* expect: discarded-futures */
  }
}
