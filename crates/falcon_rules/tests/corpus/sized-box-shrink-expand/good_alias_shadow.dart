import 'package:flutter/widgets.dart';

typedef SB = SizedBox;
typedef BoxFactory = Object Function({double? width, double? height});

Object parameterShadow(BoxFactory SB) => SB(width: 0, height: 0);

Object localShadow(BoxFactory factory) {
  final SB = factory;
  return SB(width: 0, height: 0);
}
