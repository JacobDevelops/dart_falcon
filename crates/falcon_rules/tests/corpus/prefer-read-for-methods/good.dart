import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';

class Service {
  int get value => 1;
  void save() {}
  int calculate() => 1;
}
void actions(BuildContext context) {
  context.read<Service>().save();
  Provider.of<Service>(context, listen: false).save();
  final value = context.watch<Service>().value;
  final method = context.watch<Service>().save;
  final instance = context.watch<Service>();
  context.watch<Unknown>().save();
}
class Unknown {}
class Other { Service watch() => Service(); }
void unrelated(Other context) { context.watch().save(); }
class View extends StatelessWidget {
  Widget build(BuildContext context) {
    final query = context.watch<Service>().calculate();
    final selected = context.select<Service, int>((value) => value.value).toString();
    return SizedBox();
  }
}

class DependentState extends State<StatefulWidget> {
  BuildContext get context => throw 0;
  void didChangeDependencies() {
    context.watch<Service>().save();
    Provider.of<Service>(context).save();
  }
  Widget helper(BuildContext context) {
    context.watch<Service>().save();
    Provider.of<Service>(context).save();
    return SizedBox();
  }
  Widget build(BuildContext context) => helper(context);
}
