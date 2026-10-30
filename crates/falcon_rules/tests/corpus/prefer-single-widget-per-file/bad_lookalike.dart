import 'package:flutter/widgets.dart';
const visibleForTesting = 1;
abstract class First extends StatelessWidget {}
@visibleForTesting
abstract class Testing extends StatelessWidget {} /* expect: prefer-single-widget-per-file */
