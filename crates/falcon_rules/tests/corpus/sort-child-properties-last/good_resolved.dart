import 'package:flutter/widgets.dart';

class Component {
  Component({Object? child, int priority = 0});
}

class CallbackWidget extends StatelessWidget {
  const CallbackWidget({super.key, this.child, this.onTap, this.onBuild, int priority = 0});
  final Widget? child;
  final void Function()? onTap;
  final Widget Function()? onBuild;
  static Widget create({Widget? child, int priority = 0}) => child ?? const SizedBox();
  @override
  Widget build(BuildContext context) => child ?? const SizedBox();
}

Object plain() => Component(child: Object(), priority: 1);
Widget callbacks() => CallbackWidget(
  child: const SizedBox(),
  onTap: () {},
  onBuild: () => const SizedBox(),
);

Widget staticMethod() => CallbackWidget.create(child: const SizedBox(), priority: 1);
Widget bounded<T extends Widget>(T child) => CallbackWidget(
  child: child,
  priority: 1,
);
