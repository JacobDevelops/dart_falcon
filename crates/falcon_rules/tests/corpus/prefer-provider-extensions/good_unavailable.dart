import 'package:flutter/widgets.dart';
import 'package:provider/single_child_widget.dart';
import 'package:provider/provider.dart' as p show Provider;

void examples(BuildContext context) {
  p.Provider.of<int>(context);
  p.Provider.of<int>(context, listen: false);
}
