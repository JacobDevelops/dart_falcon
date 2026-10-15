abstract class Controller {
  Future<void> dispose();
}

Future<void> close<T extends Controller>(T controller) async {
  controller.dispose(); /* expect: unawaited-futures */
}
