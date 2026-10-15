import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

class Tile extends StatelessWidget {
  const Tile({
    super.key,
    required this.title,
    required this.count,
    required this.children,
    required this.child,
    this.debugLabel,
  });

  final String title;
  final int count;
  final List<Widget> children;
  final Widget child;
  final String? debugLabel;
  final String _cache = '';

  static const String tag = 'tile';

  bool get enabled => count > 0;

  @override
  Key? get key => super.key;

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties
      ..add(StringProperty('title', title))
      ..add(IntProperty('count', count))
      ..add(StringProperty('label', label))
      ..add(FlagProperty('enabled', value: widget.enabled));
  }

  String get label => debugLabel ?? _cache;

  Tile get widget => this;

  @override
  Widget build(BuildContext context) => child;
}

class Plain {
  Plain(this.value);

  final int value;
}
