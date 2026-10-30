import 'package:flutter_bloc/flutter_bloc.dart' as fb;

class BlocBuilder<B, S> {
  const BlocBuilder();
}
class BlocConsumer<B, S> {
  const BlocConsumer();
}
class OtherBuilder {
  const OtherBuilder();
}
void examples() {
  BlocBuilder();
  new BlocBuilder<Object, int>();
  const BlocConsumer<Object, int>();
  OtherBuilder();
  fb.BlocSelector<Object, int, int>();
}
