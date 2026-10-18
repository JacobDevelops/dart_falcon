import 'package:flutter/material.dart';
class S extends State<StatefulWidget> { void initState() {
Theme.of(context); /* expect: avoid-inherited-widget-in-initstate */
MediaQuery.of(context); /* expect: avoid-inherited-widget-in-initstate */
Directionality.of(context); /* expect: avoid-inherited-widget-in-initstate */
DefaultTextStyle.of(context); /* expect: avoid-inherited-widget-in-initstate */
IconTheme.of(context); /* expect: avoid-inherited-widget-in-initstate */
}}
