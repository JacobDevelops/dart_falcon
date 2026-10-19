import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

typedef Action = FloatingActionButton;
void actions() {
  FloatingActionButton(); /* expect: prefer-define-hero-tag */
  new FloatingActionButton.small(); /* expect: prefer-define-hero-tag */
  const f.FloatingActionButton.large(); /* expect: prefer-define-hero-tag */
  FloatingActionButton.extended(label: Text('Add')); /* expect: prefer-define-hero-tag */
  Action(); /* expect: prefer-define-hero-tag */
}
