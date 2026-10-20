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
  app.View(counter); /* expect: avoid-passing-bloc-to-widget */
  new app.View(counter); /* expect: avoid-passing-bloc-to-widget */
  app.View.named(dependency: bloc); /* expect: avoid-passing-bloc-to-widget */
  ChildView(counter); /* expect: avoid-passing-bloc-to-widget */
  ViewAlias(counter); /* expect: avoid-passing-bloc-to-widget */
  app.View(app.Counter()); /* expect: avoid-passing-bloc-to-widget */
  BlocProvider.value(value: counter); /* expect: avoid-passing-bloc-to-widget */
  BlocBuilder(bloc: counter); /* expect: avoid-passing-bloc-to-widget */
  BlocListener(bloc: counter); /* expect: avoid-passing-bloc-to-widget */
  BlocConsumer(bloc: counter); /* expect: avoid-passing-bloc-to-widget */
  BlocSelector(bloc: counter); /* expect: avoid-passing-bloc-to-widget */
}
