import 'package:flutter/widgets.dart';
import 'package:meta/meta.dart';
abstract class _Private extends StatelessWidget {}
@visibleForTesting
abstract class Testing extends StatelessWidget {}
abstract class Public extends StatefulWidget {}
abstract class _AnotherPrivate extends Public {}
abstract class WidgetState extends State<Public> {}
