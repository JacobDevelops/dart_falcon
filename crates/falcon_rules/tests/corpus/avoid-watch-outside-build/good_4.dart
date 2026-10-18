import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';
import 'package:nested/nested.dart';

class DependentState extends State<View> {
  BuildContext get context => throw 0;
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    context.watch<int>();
    context.select<String, int>((value) => value.length);
    Provider.of<int>(context);
  }

  Widget build(BuildContext context) => label(context);

  Widget label(BuildContext context) => Text('${context.watch<int>()}');
}

class View extends StatefulWidget {
  State<View> createState() => DependentState();
}

class SingleView extends SingleChildStatelessWidget {
  Widget buildWithChild(BuildContext context, Widget? child) {
    context.watch<int>();
    context.select<String, int>((value) => value.length);
    Provider.of<int>(context, listen: true);
    return child ?? SizedBox();
  }
}

class SingleState extends SingleChildState<SingleChildStatefulWidget> {
  Widget buildWithChild(BuildContext context, Widget? child) {
    context.watch<int>();
    Provider.of<int>(context);
    return child ?? SizedBox();
  }
}

// Unresolved custom widget bases are not evidence of an outside-build phase.
class CustomView extends ExternalWidget {
  Widget buildWithChild(BuildContext context, Widget? child) =>
      Text('${context.watch<int>()}');
  Widget helper(BuildContext context) => Text('${Provider.of<int>(context)}');
}
