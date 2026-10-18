import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart' as p;
import 'support/other_extensions.dart';

void examples(BuildContext context) {
  p.Provider.of<int>(context);
  p.Provider.of<int>(context, listen: false);
}
