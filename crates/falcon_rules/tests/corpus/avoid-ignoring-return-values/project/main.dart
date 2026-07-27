import 'api.dart' as api;
import 'api.dart' show ValueApi, VoidApi;

void check() {
  api.calculate(); /* expect: avoid-ignoring-return-values */
  api.configure();
  ValueApi().run(); /* expect: avoid-ignoring-return-values */
  VoidApi().run();
}
