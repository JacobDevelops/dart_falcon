import 'package:flutter/material.dart' as widgets;
Object implicit() => widgets.Container(width: 10); /* expect: sized-box-for-whitespace */
Object allocated() => new widgets.Container(height: 10); /* expect: sized-box-for-whitespace */
