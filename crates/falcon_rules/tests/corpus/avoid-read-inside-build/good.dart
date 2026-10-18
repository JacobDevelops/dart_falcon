import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';

class View extends StatelessWidget {
  Widget build(BuildContext context) {
    context.watch<int>();
    Provider.of<int>(context);
    Provider.of<int>(context, listen: true);
    final callback = () => context.read<int>();
    void local() { context.read<int>(); }
    return SizedBox();
  }
  void callback(BuildContext context) { context.read<int>(); }
}
class NotAWidget {
  void build(BuildContext context) { context.read<int>(); }
}
class Other { T read<T>() => throw 0; }
void readOther(Other context) { context.read<int>(); }

class DependentState extends State<StatefulWidget> {
  BuildContext get context => throw 0;
  void didChangeDependencies() {
    context.read<int>();
    Provider.of<int>(context, listen: false);
  }
  Widget helper(BuildContext context) => Text('${context.read<int>()}');
  Widget build(BuildContext context) => helper(context);
}
