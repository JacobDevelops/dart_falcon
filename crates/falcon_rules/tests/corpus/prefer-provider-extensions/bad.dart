import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';
import 'package:provider/provider.dart' as p;
void examples(BuildContext context) {
  Provider.of<int>(context); /* expect: prefer-provider-extensions */
  Provider.of<String>(context, listen: true); /* expect: prefer-provider-extensions */
  Provider.of<Object>(context, listen: false); /* expect: prefer-provider-extensions */
  p.Provider.of<int>(context); /* expect: prefer-provider-extensions */
  p.Provider.of<String>(context, listen: (false)); /* expect: prefer-provider-extensions */
}
