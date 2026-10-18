import 'package:provider/provider.dart' as p;
class Model {
  Model();
  Model.named();
  factory Model.make() => Model();
}
void examples() {
  p.Provider.value(value: Model()); /* expect: avoid-instantiating-in-value-provider */
  p.ChangeNotifierProvider.value(value: new Model()); /* expect: avoid-instantiating-in-value-provider */
  p.ListenableProvider.value(value: Model.named()); /* expect: avoid-instantiating-in-value-provider */
  new p.Provider.value(value: (Model())); /* expect: avoid-instantiating-in-value-provider */
  p.Provider<Model>.value(value: Model.make()); /* expect: avoid-instantiating-in-value-provider */
}
