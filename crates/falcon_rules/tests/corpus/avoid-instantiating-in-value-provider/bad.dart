import 'package:provider/provider.dart' as p;
class Model {
  Model();
  Model.named();
  factory Model.make() => Model();
  void initialize() {}
}
void examples(bool flag, Model existing, Model? cached) {
  p.Provider.value(value: Model()); /* expect: avoid-instantiating-in-value-provider */
  p.ChangeNotifierProvider.value(value: new Model()); /* expect: avoid-instantiating-in-value-provider */
  p.ListenableProvider.value(value: Model.named()); /* expect: avoid-instantiating-in-value-provider */
  new p.Provider.value(value: (Model())); /* expect: avoid-instantiating-in-value-provider */
  p.Provider<Model>.value(value: Model.make()); /* expect: avoid-instantiating-in-value-provider */
  p.ChangeNotifierProvider.value(value: Model()..initialize()); /* expect: avoid-instantiating-in-value-provider */
  p.Provider.value(value: flag ? Model() : Model()); /* expect: avoid-instantiating-in-value-provider */
  p.Provider.value(value: flag ? Model() : existing); /* expect: avoid-instantiating-in-value-provider */
  p.Provider.value(value: flag ? existing : new Model()); /* expect: avoid-instantiating-in-value-provider */
  p.Provider.value(value: (flag ? existing : (Model.named()..initialize()))); /* expect: avoid-instantiating-in-value-provider */
  p.Provider.value(value: cached ?? Model()); /* expect: avoid-instantiating-in-value-provider */
  p.Provider.value(value: cached ?? (Model.named()..initialize())); /* expect: avoid-instantiating-in-value-provider */
  p.Provider.value(value: cached ?? (flag ? existing : Model())); /* expect: avoid-instantiating-in-value-provider */
  p.Provider.value(value: cached ?? (flag ? Model() : null) ?? existing); /* expect: avoid-instantiating-in-value-provider */
}
