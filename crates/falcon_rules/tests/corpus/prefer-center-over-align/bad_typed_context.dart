import 'package:flutter/widgets.dart';

Align returned(Widget child) => Align(child: child); /* expect: prefer-center-over-align */

void consume(Align box) {}

void typedContexts(Widget child) {
  final Align box = Align(child: child); /* expect: prefer-center-over-align */
  consume(Align(child: child)); /* expect: prefer-center-over-align */
  final List<Align> boxes = [Align(child: child)]; /* expect: prefer-center-over-align */
}
