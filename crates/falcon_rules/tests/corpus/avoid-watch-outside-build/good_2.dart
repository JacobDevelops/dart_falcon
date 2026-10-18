import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';

extension LocalAccess on BuildContext {
  T watch<T>() => throw 0;
  T select<T>() => throw 0;
}
void localExtension(BuildContext context) {
  context.watch<int>();
  context.select<String>();
}
