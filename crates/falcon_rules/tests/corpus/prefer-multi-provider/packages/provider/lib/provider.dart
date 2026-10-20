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
class ProxyProvider0<R> {
  ProxyProvider0({required R Function(BuildContext, R?) update, Object? child});
}
class ChangeNotifierProxyProvider0<R> {
  ChangeNotifierProxyProvider0({required R Function(BuildContext, R?) update, Object? child});
}
class ListenableProxyProvider0<R> {
  ListenableProxyProvider0({required R Function(BuildContext, R?) update, Object? child});
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
