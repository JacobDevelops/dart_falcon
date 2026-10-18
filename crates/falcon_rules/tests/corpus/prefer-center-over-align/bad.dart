import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutAlign = Align;

void examples(Widget child) {
  Align(); /* expect: prefer-center-over-align */
  const Align(); /* expect: prefer-center-over-align */
  new Align(child: child); /* expect: prefer-center-over-align */
  ui.Align(alignment: ui.Alignment.center, child: child); /* expect: prefer-center-over-align */
  const ui.Align(alignment: ui.Alignment(0, 0)); /* expect: prefer-center-over-align */
  Align(alignment: Alignment(0.0, 0), child: child); /* expect: prefer-center-over-align */
  Align(widthFactor: 2, heightFactor: 3, child: child); /* expect: prefer-center-over-align */
  Align(alignment: Alignment.center, widthFactor: 0.5, heightFactor: 2, child: child); /* expect: prefer-center-over-align */
  LayoutAlign(key: const Key('center'), child: child); /* expect: prefer-center-over-align */
  new ui.Align(alignment: new ui.Alignment(0, 0)); /* expect: prefer-center-over-align */
  const Align(widthFactor: 2, heightFactor: 3); /* expect: prefer-center-over-align */
}
