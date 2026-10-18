import 'package:flutter/widgets.dart';
class Provider<T> {
  Provider({T Function(BuildContext)? create, Object? child});
  Provider.value({T? value, Object? child});
  static T of<T>(BuildContext context, {bool listen = true}) => throw UnimplementedError();
}
class ChangeNotifierProvider<T> {
  ChangeNotifierProvider({T Function(BuildContext)? create, Object? child});
  ChangeNotifierProvider.value({T? value, Object? child});
}
class ListenableProvider<T> {
  ListenableProvider({T Function(BuildContext)? create, Object? child});
  ListenableProvider.value({T? value, Object? child});
}
class MultiProvider {
  MultiProvider({List<Object>? providers, Object? child});
}
extension ReadContext on BuildContext {
  T read<T>() => throw UnimplementedError();
}
extension WatchContext on BuildContext {
  T watch<T>() => throw UnimplementedError();
}
