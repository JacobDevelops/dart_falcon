import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_bloc/flutter_bloc.dart' as fb;

void examples() {
  BlocBuilder(buildWhen: (previous, current) => previous != current);
  new BlocBuilder<Object, int>(buildWhen: null);
  const BlocBuilder(buildWhen: null);
  fb.BlocConsumer(buildWhen: (previous, current) => true);
  const fb.BlocConsumer<Object, int>(buildWhen: null);
  BlocSelector(selector: (state) => state);
  new fb.BlocSelector<Object, int, int>();
}
void shadowed(dynamic BlocBuilder, dynamic BlocConsumer) {
  BlocBuilder();
  BlocConsumer();
}
void shadowedPrefix(dynamic fb) {
  fb.BlocBuilder();
  fb.BlocConsumer();
}
class MemberShadow {
  dynamic BlocBuilder;
  void build() { BlocBuilder(); }
}
