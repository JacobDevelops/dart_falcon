import 'package:flutter/widgets.dart';
void examples(dynamic alignment, dynamic key, dynamic axis) {
  SingleChildScrollView(child: Column(mainAxisAlignment: alignment, children: [Text('a'), Text('b')]));
  SingleChildScrollView(child: Column(mainAxisSize: alignment, children: [Text('a'), Text('b')]));
  SingleChildScrollView(child: Column(crossAxisAlignment: alignment, children: [Text('a'), Text('b')]));
  SingleChildScrollView(child: Column(key: key, children: [Text('a'), Text('b')]));
  SingleChildScrollView(scrollDirection: axis, child: Column(children: [Text('a'), Text('b')]));
}
