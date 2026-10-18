import 'package:flutter_bloc/flutter_bloc.dart' as fb;
class BlocProvider { static int of(Object context) => 0; }
class Custom { int of(Object context) => 0; }
void example(Object context, Custom service, Custom BlocProvider, bool listen) {
  BlocProvider.of(context);
  service.of(context);
  context.read<int>();
  context.watch<int>();
  fb.BlocProvider.of<int>(context, listen: listen);
}
extension LocalContext on Object {
  T read<T>() => throw StateError('fixture');
  T watch<T>() => throw StateError('fixture');
}
