import 'package:flutter/widgets.dart';

typedef Cont = Container;
typedef BoxFactory = Object Function({Decoration? decoration, Widget? child});

Object parameterShadow(BoxFactory Cont, Decoration d, Widget c) =>
    Cont(decoration: d, child: c);

Object localShadow(BoxFactory factory, Decoration d, Widget c) {
  final Cont = factory;
  return Cont(decoration: d, child: c);
}
