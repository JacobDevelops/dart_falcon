import 'package:flutter/material.dart';

class LocalWidget extends StatelessWidget {
  const LocalWidget({super.key});

  @override
  Widget build(BuildContext context) => const SizedBox();
}

Widget wrap(AlignmentGeometry value) => Container(alignment: value, child: const LocalWidget()); /* expect: prefer-align-over-container */
