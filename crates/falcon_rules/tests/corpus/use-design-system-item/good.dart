import 'package:flutter/material.dart';

Widget build(BuildContext context) {
  return AppContainer(
    child: AppText('hello'),
  );
}

class MyWidget {
  Widget make() => AppButton(label: 'ok');
}
