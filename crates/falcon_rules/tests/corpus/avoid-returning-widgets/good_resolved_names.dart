import 'package:flutter/widgets.dart' as flutter;

class Widget {}
class StatelessWidget {}
class StatefulWidget {}
class Future<T> {}
class Stream<T> {}
class Completer<T> {}

Widget localWidget() => Widget();
StatelessWidget localStateless() => StatelessWidget();
StatefulWidget localStateful() => StatefulWidget();
Future<flutter.Widget> localFuture() => Future<flutter.Widget>();
Stream<flutter.Widget> localStream() => Stream<flutter.Widget>();
Completer<flutter.Widget> localCompleter() => Completer<flutter.Widget>();

class LocalMembers {
  Widget method() => Widget();
  Widget get value => Widget();
}

Widget genericWidget<Widget>(Widget value) => value;

class GenericMembers<Widget> {
  Widget method(Widget value) => value;
  Widget get value => throw UnimplementedError();
}
