import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';

class View extends StatelessWidget {
  Widget build(BuildContext context) {
    context.watch<int>();
    context.select<String, int>((value) => value.length);
    Provider.of<int>(context);
    Provider.of<int>(context, listen: true);
    return SizedBox();
  }
  // Helpers can establish dependencies while called synchronously from build.
  Widget label(BuildContext context) => Text('${context.watch<int>()}');
  void action(BuildContext context) {
    context.watch<int>();
  }
}
void callback(BuildContext context, bool listen) {
  context.read<int>();
  Provider.of<int>(context, listen: false);
  Provider.of<int>(context, listen: listen);
}
class Other { T watch<T>() => throw 0; }
void unrelated(Other context) { context.watch<int>(); }
