import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';

void action(BuildContext context) {
  context.watch<int>(); /* expect: avoid-watch-outside-build */
  context.select<String, int>((value) => value.length); /* expect: avoid-watch-outside-build */
  Provider.of<String>(context); /* expect: avoid-watch-outside-build */
  Provider.of<int>(context, listen: true); /* expect: avoid-watch-outside-build */
}
class View extends StatelessWidget {
  Widget build(BuildContext context) {
    final callback = () => context.watch<int>(); /* expect: avoid-watch-outside-build */
    void local() { context.watch<String>(); } /* expect: avoid-watch-outside-build */
    return SizedBox();
  }
}

class PlainService {
  void action(BuildContext context) {
    context.watch<int>(); /* expect: avoid-watch-outside-build */
  }
  void build(BuildContext context) {
    Provider.of<int>(context); /* expect: avoid-watch-outside-build */
  }
}

class ViewState extends State<StatefulWidget> {
  BuildContext get context => throw 0;
  void initState() {
    context.watch<int>(); /* expect: avoid-watch-outside-build */
  }
  void dispose() {
    Provider.of<int>(context); /* expect: avoid-watch-outside-build */
  }
  Widget build(BuildContext context) => GestureDetector(
    onTap: () {
      context.watch<int>(); /* expect: avoid-watch-outside-build */
    },
  );
}
