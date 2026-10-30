import 'package:flutter/widgets.dart' as f;
abstract class First extends f.StatelessWidget {}
abstract class Second extends f.StatefulWidget {} /* expect: prefer-single-widget-per-file */
abstract class _Private extends f.StatelessWidget {} /* expect: prefer-single-widget-per-file */
abstract class Indirect extends First {} /* expect: prefer-single-widget-per-file */
abstract class Render extends f.RenderObjectWidget {} /* expect: prefer-single-widget-per-file */
abstract class Direct extends f.Widget {} /* expect: prefer-single-widget-per-file */
