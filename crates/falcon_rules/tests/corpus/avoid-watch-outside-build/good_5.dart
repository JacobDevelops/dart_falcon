import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';

class GetterState extends State<StatefulWidget> {
  BuildContext get context => throw 0;
  Widget get label => Text('${context.watch<int>()}');
  int operator [](int index) => context.watch<List<int>>()[index];
  set dependency(int value) {
    context.watch<int>();
  }
  Widget build(BuildContext context) {
    dependency = 1;
    final value = this[0];
    return label;
  }
}

class UnknownState extends GeneratedState {
  BuildContext get context => throw 0;
  Widget get label => Text('${context.watch<int>()}');
}
