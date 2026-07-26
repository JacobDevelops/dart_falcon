import 'package:flutter/material.dart' as f;
import 'base.dart';

class BadWidget extends BaseWidget {
  @override
  f.State<BadWidget> createState() => _BadState(this); /* expect: no-logic-in-create-state */
}

class GoodWidget extends BaseWidget {
  @override
  f.State<GoodWidget> createState() => _GoodState.named();
}

class _BadState extends f.State<BadWidget> {
  _BadState(Object widget);
}

class _GoodState extends f.State<GoodWidget> {
  _GoodState.named();
}
