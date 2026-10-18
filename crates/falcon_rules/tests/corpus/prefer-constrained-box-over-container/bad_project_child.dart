import 'package:flutter/material.dart';

class LocalWidget extends StatelessWidget {
  const LocalWidget({super.key});

  @override
  Widget build(BuildContext context) => const SizedBox();
}

Widget wrap(BoxConstraints value) => Container(constraints: value, child: const LocalWidget()); /* expect: prefer-constrained-box-over-container */
