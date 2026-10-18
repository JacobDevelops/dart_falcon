import 'package:flutter/widgets.dart';
extension OtherContext on BuildContext {
  T read<T>() => throw StateError('other read');
  T watch<T>() => throw StateError('other watch');
}
