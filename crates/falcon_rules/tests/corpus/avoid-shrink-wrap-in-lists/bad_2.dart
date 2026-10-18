import 'package:flutter/material.dart';

Widget parenthesized() => Column(children: [
  (ListView(shrinkWrap: true)), /* expect: avoid-shrink-wrap-in-lists */
]);
