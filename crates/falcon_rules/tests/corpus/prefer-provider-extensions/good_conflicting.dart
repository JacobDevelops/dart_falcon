import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart' as p;

// A same-library extension takes precedence over imported provider extensions.
extension LocalRead on BuildContext {
  T read<T>() => throw StateError('local read');
  T watch<T>() => throw StateError('local watch');
}
void examples(BuildContext context) {
  p.Provider.of<int>(context);
  p.Provider.of<int>(context, listen: false);
}
