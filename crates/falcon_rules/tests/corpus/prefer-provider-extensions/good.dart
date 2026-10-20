import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';
import 'package:provider/provider.dart' as p;
void examples(BuildContext context, bool listen) {
  context.read<int>();
  context.watch<int>();
  Provider.of<int>(context, listen: listen);
  p.Provider<int>(create: (_) => 1);
  p.Provider.value(value: 1);
}
void shadowed(BuildContext context, dynamic Provider) {
  Provider.of<int>(context);
}
void shadowedPrefix(BuildContext context, dynamic p) {
  p.Provider.of<int>(context);
}
