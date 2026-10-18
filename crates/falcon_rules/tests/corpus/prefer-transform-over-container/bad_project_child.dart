import 'package:flutter/material.dart';

class LocalWidget extends StatelessWidget {
  const LocalWidget({super.key});

  @override
  Widget build(BuildContext context) => const SizedBox();
}

Widget wrap(Matrix4 value) => Container(transform: value, child: const LocalWidget()); /* expect: prefer-transform-over-container */
