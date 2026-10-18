import 'package:flutter/material.dart';

class LocalWidget extends StatelessWidget {
  const LocalWidget({super.key});

  @override
  Widget build(BuildContext context) => const SizedBox();
}

Widget wrap(EdgeInsetsGeometry value) => Container(padding: value, child: const LocalWidget()); /* expect: prefer-padding-over-container */
