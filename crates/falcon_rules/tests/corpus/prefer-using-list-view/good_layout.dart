import 'package:flutter/widgets.dart';
void examples(dynamic alignment, dynamic key, dynamic axis) {
  SingleChildScrollView(child: Column(mainAxisAlignment: alignment));
  SingleChildScrollView(child: Column(mainAxisSize: alignment));
  SingleChildScrollView(child: Column(crossAxisAlignment: alignment));
  SingleChildScrollView(child: Column(key: key));
  SingleChildScrollView(scrollDirection: axis, child: Column());
}
