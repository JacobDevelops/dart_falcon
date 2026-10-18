import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart';

void helper(bool enabled) {
  if (enabled) useState(0);
}

void useStable(bool enabled) {
  useEffect(() {
    if (enabled) useState(0);
  });
}

class PlainWidget extends StatelessWidget {
  final bool enabled;
  const PlainWidget(this.enabled);

  @override
  Widget build(BuildContext context) {
    if (enabled) useState(0);
    return const SizedBox();
  }
}
