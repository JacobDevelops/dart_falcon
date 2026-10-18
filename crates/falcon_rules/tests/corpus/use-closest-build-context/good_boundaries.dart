import 'package:flutter/material.dart';
void f(BuildContext outer) { final callback = (int value) { use(outer); }; }
void g(BuildContext context) { final callback = (BuildContext context) { use(context); }; }

