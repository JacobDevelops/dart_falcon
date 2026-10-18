import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart' as p;

void example(BuildContext context) {
  p.Provider.of<int>(context); /* expect: prefer-provider-extensions */
}
