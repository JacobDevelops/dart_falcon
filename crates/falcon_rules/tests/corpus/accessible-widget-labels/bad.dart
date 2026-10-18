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
}
