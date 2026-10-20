import 'package:provider/provider.dart' as p;
class Model {
  Model();
  static final Model instance = Model();
  static Model cached() => instance;
  void initialize() {}
}
class Provider {
  Provider.value({Object? value});
}
Model makeModel() => Model();
void examples(Model existing, dynamic factory, bool flag, Model? cached) {
  p.Provider.value(value: existing);
  p.ChangeNotifierProvider.value(value: existing);
  p.ListenableProvider.value(value: existing);
  p.Provider(create: (_) => Model());
  Provider.value(value: Model());
  p.Provider.value(value: makeModel());
  p.Provider.value(value: factory());
  p.Provider.value(value: Model.cached());
  p.Provider.value(value: existing..initialize());
  p.Provider.value(value: flag ? existing : Model.instance);
  p.Provider.value(value: (flag ? existing : Model.cached())..initialize());
  p.Provider.value(value: flag ? makeModel() : Model.cached());
  p.Provider.value(value: cached ?? existing);
  p.Provider.value(value: cached ?? Model.cached());
}
void shadowed(dynamic Model) {
  p.Provider.value(value: Model());
}
