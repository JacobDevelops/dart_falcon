import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void actions(Object tag) {
  FloatingActionButton(heroTag: 'add');
  const FloatingActionButton.small(heroTag: 1);
  f.FloatingActionButton.large(heroTag: tag);
  FloatingActionButton.extended(label: Text('Add'), heroTag: null);
  FloatingActionButton(heroTag: (null));
  Hero(tag: 'flight');
}
void shadow(dynamic FloatingActionButton) { FloatingActionButton(); }
