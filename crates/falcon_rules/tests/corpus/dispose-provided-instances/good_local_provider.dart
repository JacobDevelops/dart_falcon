import 'package:riverpod/riverpod.dart';
class Provider<T> { Provider(T Function(Object) create); }
class Service { Service(); void dispose() {} void close() {} }
final localProvider = Provider((ref) => Service());
