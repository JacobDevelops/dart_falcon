import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef V = SingleChildScrollView;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  ListView(children: [child]);
  SingleChildScrollView(child: Row());
  SingleChildScrollView(child: child);
  Column(children: [child]);
  SingleChildScrollView();
  SingleChildScrollView(child: Column());
  const SingleChildScrollView(child: Column(children: []));
  new SingleChildScrollView(child: Column(children: [child]));
  f.SingleChildScrollView(child: f.Column(children: <Widget>[]));
  V(child: Column(children: [child]));
  SingleChildScrollView(child: Column(children: ([child])));
  SingleChildScrollView(child: Column(children: <Widget>[child]));
  SingleChildScrollView(child: Column(children: [...<Widget>[]]));
  SingleChildScrollView(child: Column(children: [...<Widget>[child]]));
  SingleChildScrollView(child: Column(children: [if (size > 0) child]));
  SingleChildScrollView(child: Column(children: [if (size > 0) child else child]));
  SingleChildScrollView(child: Column(children: [for (final item in <Widget>[]) item]));
  SingleChildScrollView(child: Column(children: [child, ...<Widget>[]]));
  SingleChildScrollView(child: Column(children: [child, if (size > 0) child]));
  SingleChildScrollView(child: Column(children: image));
  SingleChildScrollView(child: Column(children: image ? <Widget>[] : <Widget>[child]));
  SingleChildScrollView(child: Column(children: [child, ...image]));
  SingleChildScrollView(child: Column(children: [child, ...?image]));
  SingleChildScrollView(child: Column(children: [child, ?image]));
  SingleChildScrollView(child: Column(children: [for (final item in <Widget>[]) item, child]));
}
