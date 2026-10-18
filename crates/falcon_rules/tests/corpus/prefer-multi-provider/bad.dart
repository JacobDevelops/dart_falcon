import 'package:provider/provider.dart' as p;
void examples(Object existing) {
  p.Provider(create: (_) => 1, child: p.Provider(create: (_) => 2)); /* expect: prefer-multi-provider */
  p.Provider.value(value: existing, child: p.ChangeNotifierProvider.value(value: existing)); /* expect: prefer-multi-provider */
  p.ChangeNotifierProvider(create: (_) => existing, child: p.ListenableProvider.value(value: existing)); /* expect: prefer-multi-provider */
  new p.ListenableProvider(create: (_) => existing, child: new p.Provider(create: (_) => 1)); /* expect: prefer-multi-provider */
  p.Provider<int>(create: (_) => 1, child: (p.Provider<String>(create: (_) => 'a'))); /* expect: prefer-multi-provider */
  p.Provider<Object>.value(value: existing, child: p.ChangeNotifierProvider<Object>.value(value: existing)); /* expect: prefer-multi-provider */
}
