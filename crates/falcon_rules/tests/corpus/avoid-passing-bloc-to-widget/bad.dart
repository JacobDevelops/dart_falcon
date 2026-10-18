import 'support.dart' as app;
import 'package:bloc/bloc.dart' as b;

typedef ViewAlias = app.View;
class ChildView extends app.View {
  ChildView(Object dependency) : super(dependency);
}
class BlocProvider extends app.View {
  BlocProvider.value({required Object value}) : super(value);
}
class BlocBuilder extends app.View {
  BlocBuilder({required Object bloc}) : super(bloc);
}
class BlocListener extends app.View {
  BlocListener({required Object bloc}) : super(bloc);
}
class BlocConsumer extends app.View {
  BlocConsumer({required Object bloc}) : super(bloc);
}
class BlocSelector extends app.View {
  BlocSelector({required Object bloc}) : super(bloc);
}

void dependencies(app.Counter counter, b.Bloc<Object, int> bloc) {
  app.View(/* expect: avoid-passing-bloc-to-widget */ counter);
  new app.View(/* expect: avoid-passing-bloc-to-widget */ counter);
  app.View.named(dependency: /* expect: avoid-passing-bloc-to-widget */ bloc);
  ChildView(/* expect: avoid-passing-bloc-to-widget */ counter);
  ViewAlias(/* expect: avoid-passing-bloc-to-widget */ counter);
  app.View(/* expect: avoid-passing-bloc-to-widget */ app.Counter());
  BlocProvider.value(value: /* expect: avoid-passing-bloc-to-widget */ counter);
  BlocBuilder(bloc: /* expect: avoid-passing-bloc-to-widget */ counter);
  BlocListener(bloc: /* expect: avoid-passing-bloc-to-widget */ counter);
  BlocConsumer(bloc: /* expect: avoid-passing-bloc-to-widget */ counter);
  BlocSelector(bloc: /* expect: avoid-passing-bloc-to-widget */ counter);
}
