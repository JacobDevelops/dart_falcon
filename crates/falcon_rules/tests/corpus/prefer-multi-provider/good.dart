import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart' as p;
class Provider { Provider({Object? child}); }
class Wrapper { Wrapper({Object? child}); }
void examples(Object existing, Object child) {
  p.Provider(create: (_) => 1);
  p.Provider.value(value: existing, child: child);
  p.MultiProvider(providers: [p.Provider(create: (_) => 1), p.Provider(create: (_) => 2)], child: child);
  p.Provider(create: (_) => 1, child: Wrapper(child: p.Provider(create: (_) => 2)));
  Provider(child: p.Provider(create: (_) => 1));
  p.Provider(create: (_) => 1, child: Provider(child: child));
  p.Provider(create: (_) => 1, child: p.MultiProvider(providers: [], child: child));
}
void shadowed(dynamic p) {
  p.Provider(child: p.Provider());
}
void lookupChild(BuildContext context) {
  p.Provider(create: (_) => 1, child: p.Provider.of<Object>(context));
}
