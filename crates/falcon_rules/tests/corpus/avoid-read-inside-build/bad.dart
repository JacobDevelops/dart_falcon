import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';
import 'package:nested/nested.dart';

class View extends StatelessWidget {
  Widget build(BuildContext context) {
    context.read<int>(); /* expect: avoid-read-inside-build */
    final value = (context.read<String>()); /* expect: avoid-read-inside-build */
    if (true) context.read<bool>(); /* expect: avoid-read-inside-build */
    Provider.of<int>(context, listen: false); /* expect: avoid-read-inside-build */
    return context.read<Widget>(); /* expect: avoid-read-inside-build */
  }
}
class StatefulView extends StatefulWidget {
  State<StatefulView> createState() => ViewState();
}
class ViewState extends State<StatefulView> {
  Widget build(BuildContext other) {
    return other.read<Widget>(); /* expect: avoid-read-inside-build */
  }
}

class SingleView extends SingleChildStatelessWidget {
  Widget buildWithChild(BuildContext context, Widget? child) {
    context.read<int>(); /* expect: avoid-read-inside-build */
    Provider.of<int>(context, listen: false); /* expect: avoid-read-inside-build */
    return child ?? SizedBox();
  }
}

class SingleState extends SingleChildState<SingleChildStatefulWidget> {
  Widget buildWithChild(BuildContext context, Widget? child) {
    context.read<int>(); /* expect: avoid-read-inside-build */
    Provider.of<int>(context, listen: false); /* expect: avoid-read-inside-build */
    return child ?? SizedBox();
  }
}
