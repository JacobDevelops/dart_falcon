import 'package:flutter/material.dart';
class S extends State<StatefulWidget> { void didChangeDependencies() {
Theme.of(context);
MediaQuery.of(context);
Directionality.of(context);
DefaultTextStyle.of(context);
IconTheme.of(context);
}}
