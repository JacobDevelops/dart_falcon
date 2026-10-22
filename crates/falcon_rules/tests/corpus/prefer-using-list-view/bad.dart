import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef V = SingleChildScrollView;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  SingleChildScrollView(child: Column(children: [child, child])); /* expect: prefer-using-list-view */
  const SingleChildScrollView(child: Column(children: [Text('a'), Text('b')])); /* expect: prefer-using-list-view */
  new SingleChildScrollView(child: Column(children: [child, child])); /* expect: prefer-using-list-view */
  f.SingleChildScrollView(child: f.Column(children: [child, child])); /* expect: prefer-using-list-view */
  V(child: Column(children: [child, child])); /* expect: prefer-using-list-view */
  SingleChildScrollView(child: Column(children: [Text('a'), Text('much longer')])); /* expect: prefer-using-list-view, msg: "Consider `ListView` for this scrollable `Column`; preserve child alignment and padding explicitly, including `ListView`'s default `MediaQuery` padding." */
  SingleChildScrollView(child: Column(children: ([child, child]))); /* expect: prefer-using-list-view */
  SingleChildScrollView(child: Column(children: [child, ...<Widget>[], child])); /* expect: prefer-using-list-view */
  SingleChildScrollView(child: Column(children: [child, if (size > 0) child, child])); /* expect: prefer-using-list-view */
  SingleChildScrollView(child: Column(children: [child, for (final item in <Widget>[]) item, child])); /* expect: prefer-using-list-view */
  SingleChildScrollView(child: Column(children: <Widget>[child, child])); /* expect: prefer-using-list-view */
}
