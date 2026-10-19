import 'package:flutter/widgets.dart';

double counter = 0;
double next() => counter++;
double get changing => counter++;
class Sizes {
  double get size => counter++;
  double operator [](int index) => counter++;
}
void examples(Sizes sizes) {
  SizedBox(width: next(), height: next());
  SizedBox(width: changing, height: changing);
  SizedBox(width: sizes.size, height: sizes.size);
  SizedBox(width: sizes[0], height: sizes[0]);
  SizedBox(width: counter++, height: counter++);
}
