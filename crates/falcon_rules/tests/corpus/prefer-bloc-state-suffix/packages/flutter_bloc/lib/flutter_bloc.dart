export 'package:bloc/bloc.dart';
class BlocProvider<T> {
  BlocProvider({required T Function(Object) create, required Object child, bool lazy = true});
  BlocProvider.value({required T value, required Object child});
  static T of<T>(Object context, {bool listen = false}) => throw StateError('fixture');
}
class RepositoryProvider<T> {
  RepositoryProvider({required T Function(Object) create, required Object child});
  static T of<T>(Object context, {bool listen = false}) => throw StateError('fixture');
}
class MultiBlocProvider { MultiBlocProvider({required List<Object> providers, required Object child}); }
extension BlocContext on Object {
  T read<T>() => throw StateError('fixture');
  T watch<T>() => throw StateError('fixture');
}
