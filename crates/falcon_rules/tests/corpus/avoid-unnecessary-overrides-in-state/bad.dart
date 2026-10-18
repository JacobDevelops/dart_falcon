import 'package:flutter/material.dart';

class Redundant extends State<StatefulWidget> {
  @override
  void initState() => super.initState(); /* expect: avoid-unnecessary-overrides-in-state */
  @override
  void dispose() { super.dispose(); } /* expect: avoid-unnecessary-overrides-in-state */
  @override
  void didChangeDependencies() => super.didChangeDependencies(); /* expect: avoid-unnecessary-overrides-in-state */
  @override
  void deactivate() => super.deactivate(); /* expect: avoid-unnecessary-overrides-in-state */
  @override
  void reassemble() => super.reassemble(); /* expect: avoid-unnecessary-overrides-in-state */
}
