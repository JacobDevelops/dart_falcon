import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart' as w;

Widget zeroInts() {
  return SizedBox(height: 0, width: 0); /* expect: sized-box-shrink-expand */
}

Widget zeroDoublesWithChild(Widget logo) {
  return SizedBox( /* expect: sized-box-shrink-expand */
    width: 0.0,
    height: 0.0,
    child: logo,
  );
}

Widget infinite(Widget logo) {
  return SizedBox( /* expect: sized-box-shrink-expand */
    height: double.infinity,
    width: double.infinity,
    child: logo,
  );
}

Widget withKey() {
  return SizedBox(key: const Key('k'), width: 0x0, height: 0); /* expect: sized-box-shrink-expand */
}

Widget prefixed() {
  return w.SizedBox(width: double.infinity, height: double.infinity); /* expect: sized-box-shrink-expand */
}

Widget explicitNew() {
  return new SizedBox(width: 0, height: 0); /* expect: sized-box-shrink-expand */
}

Widget constant() {
  return const SizedBox(width: 0, height: 0); /* expect: sized-box-shrink-expand */
}
