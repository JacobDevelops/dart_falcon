import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';
import 'package:flutter_hooks/flutter_hooks.dart';

class HookView extends HookWidget {
  Widget build(BuildContext context) => Text('${context.watch<int>()}');
}
class View extends StatelessWidget {
  Widget build(BuildContext context) => Builder(
    builder: ((BuildContext childContext) => Text('${childContext.watch<int>()}')),
  );
}
class GeneratedView extends StatelessWidget {
  Widget build(BuildContext context) {
    final widgets = List.generate(2, (index) => Text('${context.watch<int>()}'));
    final mapped = [1, 2].map((value) => Text('${context.watch<int>()}'));
    final delegate = SliverChildBuilderDelegate(
      (BuildContext childContext, int index) => Text('${childContext.watch<int>()}'),
    );
    return Column(children: widgets);
  }
}
