import 'support.dart' as real;
import 'package:flutter_bloc/flutter_bloc.dart' as framework;
import 'package:flutter/widgets.dart' as flutter;

class Widget {
  Widget(Object dependency);
}
class LocalView extends Widget {
  LocalView(Object dependency) : super(dependency);
}
class Bloc<E, S> {}
class Cubit<S> {}

class LookupView extends real.View {
  LookupView() : super(0);
  static LookupView of(real.Counter counter) => LookupView();
}

void boundaries(real.Counter counter, dynamic unknown) {
  Widget(counter);
  LocalView(counter);
  real.Repository(counter);
  real.View(Bloc<Object, int>());
  real.View(Cubit<int>());
  real.View(real.Repository(0));
  real.View(unknown);
  LookupView.of(counter);
  framework.BlocProvider.value(value: counter, child: flutter.SizedBox());
  framework.RepositoryProvider.value(value: counter, child: flutter.SizedBox());
  framework.BlocBuilder<real.Counter, int>(
    bloc: counter,
    builder: (context, state) => flutter.SizedBox(),
  );
  framework.BlocListener<real.Counter, int>(
    bloc: counter,
    listener: (context, state) {},
    child: flutter.SizedBox(),
  );
  framework.BlocConsumer<real.Counter, int>(
    bloc: counter,
    builder: (context, state) => flutter.SizedBox(),
    listener: (context, state) {},
  );
  framework.BlocSelector<real.Counter, int, bool>(
    bloc: counter,
    selector: (state) => state > 0,
    builder: (context, selected) => flutter.SizedBox(),
  );
  final View = (Object dependency) => real.Repository(dependency);
  View(counter);
}
