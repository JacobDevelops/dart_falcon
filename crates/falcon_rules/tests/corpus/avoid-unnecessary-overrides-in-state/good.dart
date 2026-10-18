import 'package:flutter/material.dart';
import 'package:meta/meta.dart';

class Meaningful extends State<StatefulWidget> {
  @override
  void initState() {
    // Initialization will be added here intentionally.
    super.initState();
  }
  @override
  void dispose() { print('cleanup'); super.dispose(); }
  /// Preserve this lifecycle documentation.
  @override
  void didChangeDependencies() => super.didChangeDependencies();
  @override
  void deactivate() {}
  @deprecated
  @override
  void reassemble() => super.reassemble();
}
class Parent {
  void dispose() {}
}
class NotState extends Parent {
  @override
  void dispose() => super.dispose();
}

class StateBase extends State<StatefulWidget> {
  void ordinary() {}
}
class OrdinaryState extends StateBase {
  // Owned by unnecessary-overrides, not this rule: no protected mismatch.
  @override
  void ordinary() => super.ordinary();
}

class ProtectedState extends State<StatefulWidget> {
  // Keeping protected is also owned by unnecessary-overrides.
  @protected
  @override
  void initState() => super.initState();
}
