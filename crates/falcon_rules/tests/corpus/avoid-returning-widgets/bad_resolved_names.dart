import 'dart:async' as async;
import 'package:flutter/widgets.dart' as flutter;

typedef WidgetAlias = flutter.Widget;
typedef FutureAlias<T> = async.Future<T>;

flutter.Widget widget() => throw UnimplementedError(); /* expect: avoid-returning-widgets */
WidgetAlias alias() => throw UnimplementedError(); /* expect: avoid-returning-widgets */
async.Future<flutter.Widget> future() => throw UnimplementedError(); /* expect: avoid-returning-widgets */
FutureAlias<flutter.Widget> futureAlias() => throw UnimplementedError(); /* expect: avoid-returning-widgets */
async.Stream<flutter.Widget> stream() => throw UnimplementedError(); /* expect: avoid-returning-widgets */
async.Completer<flutter.Widget> completer() => throw UnimplementedError(); /* expect: avoid-returning-widgets */

class Members {
  flutter.Widget method() => throw UnimplementedError(); /* expect: avoid-returning-widgets */
  flutter.Widget get value => throw UnimplementedError(); /* expect: avoid-returning-widgets */
}
