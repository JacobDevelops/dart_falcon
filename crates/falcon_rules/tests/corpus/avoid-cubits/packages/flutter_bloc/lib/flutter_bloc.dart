export 'package:bloc/bloc.dart';

class BlocBuilder<B, S> {
  const BlocBuilder({Object? builder, Object? buildWhen});
}
class BlocConsumer<B, S> {
  const BlocConsumer({Object? listener, Object? builder, Object? buildWhen});
}
class BlocSelector<B, S, T> {
  const BlocSelector({Object? selector, Object? builder});
}
