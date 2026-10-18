import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as m;

Widget standalone() => ListView(shrinkWrap: true);
Widget falseFlag() => Column(children: [ListView(shrinkWrap: false)]);
Widget unknownFlag(bool flag) => Column(children: [ListView(shrinkWrap: flag)]);
Widget noFlag() => Row(children: [ListView()]);
Widget unrelatedParent() => Center(child: ListView(shrinkWrap: true));
Widget localShadow(dynamic ListView) => Column(children: [ListView(shrinkWrap: true)]);
Widget prefixShadow(dynamic m) => Column(children: [m.ListView(shrinkWrap: true)]);
Widget deferred() => Column(children: [Builder(builder: (context) => ListView(shrinkWrap: true))]);
