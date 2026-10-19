import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void actions() {
  GestureDetector(onTap: () {}); /* expect: accessible-widget-labels */
  f.InkWell(onTap: () {}, child: Icon(null)); /* expect: accessible-widget-labels */
  InkResponse(onTap: () {}, child: Text(' ')); /* expect: accessible-widget-labels */
  GestureDetector(onTap: () {}, child: Icon(null)); /* expect: accessible-widget-labels */
  InkWell(onTap: () {}, child: Text('')); /* expect: accessible-widget-labels */
  InkResponse(onTap: () {}); /* expect: accessible-widget-labels */
  Tooltip(message: '', child: InkWell(onTap: () {})); /* expect: accessible-widget-labels */
  Tooltip(richMessage: null, child: InkWell(onTap: () {})); /* expect: accessible-widget-labels */
  Tooltip(richMessage: (null), child: InkResponse(onTap: () {})); /* expect: accessible-widget-labels */
  Semantics(attributedLabel: null, child: InkWell(onTap: () {})); /* expect: accessible-widget-labels */
  Semantics(attributedLabel: (null), child: InkResponse(onTap: () {})); /* expect: accessible-widget-labels */
}

void shadowed(dynamic Tooltip, dynamic Semantics) {
  Tooltip(richMessage: 'Delete', child: InkWell(onTap: () {})); /* expect: accessible-widget-labels */
  Semantics(attributedLabel: 'Delete', child: InkResponse(onTap: () {})); /* expect: accessible-widget-labels */
}

void siblings(dynamic richMessage, dynamic attributedLabel) {
  Tooltip(richMessage: richMessage, child: InkWell(onTap: () {}));
  InkWell(onTap: () {}); /* expect: accessible-widget-labels */
  Semantics(attributedLabel: attributedLabel, child: InkResponse(onTap: () {}));
  InkResponse(onTap: () {}); /* expect: accessible-widget-labels */
}
