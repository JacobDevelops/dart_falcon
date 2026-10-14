import 'package:flutter/material.dart';

Widget build(BuildContext context) {
  return AppContainer(
    child: AppText('hello'),
  );
}

class MyWidget {
  Widget make() => AppButton(label: 'ok');
}

final namedConstructor = Container.named();
final prefixedCall = framework.Container();

class Initializers {
  final named = Container.named();
  final replacement = AppContainer();
}
