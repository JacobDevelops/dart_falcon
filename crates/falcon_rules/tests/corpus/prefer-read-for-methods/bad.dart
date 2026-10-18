import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';
import 'package:nested/nested.dart';

class Service {
  void save() {}
  void refresh() {}
  void reset() {}
}
class View extends StatelessWidget {
  Widget build(BuildContext context) {
    context.watch<Service>().save(); /* expect: prefer-read-for-methods */
    context.watch<Service>().refresh(); /* expect: prefer-read-for-methods */
    context.watch<Service>().reset(); /* expect: prefer-read-for-methods */
    Provider.of<Service>(context).save(); /* expect: prefer-read-for-methods */
    Provider.of<Service>(context, listen: true).refresh(); /* expect: prefer-read-for-methods */
    return SizedBox();
  }
}

class SingleView extends SingleChildStatelessWidget {
  Widget buildWithChild(BuildContext context, Widget? child) {
    context.watch<Service>().save(); /* expect: prefer-read-for-methods */
    Provider.of<Service>(context).refresh(); /* expect: prefer-read-for-methods */
    return child ?? SizedBox();
  }
}

class SingleState extends SingleChildState<SingleChildStatefulWidget> {
  Widget buildWithChild(BuildContext context, Widget? child) {
    context.watch<Service>().save(); /* expect: prefer-read-for-methods */
    Provider.of<Service>(context).refresh(); /* expect: prefer-read-for-methods */
    return child ?? SizedBox();
  }
}
