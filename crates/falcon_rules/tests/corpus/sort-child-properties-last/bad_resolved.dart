import 'package:flutter/widgets.dart';

class customWidget extends StatelessWidget {
  const customWidget({super.key, this.child, this.onTap, this.value = 0});
  final Widget? child;
  final void Function()? onTap;
  final int value;
  @override
  Widget build(BuildContext context) => child ?? const SizedBox();
}

Widget misplaced() => customWidget(
  child: const SizedBox(), /* expect: sort-child-properties-last */
  onTap: () {},
  value: 1,
);

typedef WidgetAlias = customWidget;
Widget aliasedAndParenthesized() => (WidgetAlias(
  child: const SizedBox(), /* expect: sort-child-properties-last */
  value: 1,
));
