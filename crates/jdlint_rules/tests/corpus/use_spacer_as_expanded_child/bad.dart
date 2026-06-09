import 'package:flutter/material.dart';

// Bad: Expanded with empty Container or SizedBox

class BadLayout1 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Text('Top'),
        Expanded( /* expect: use_spacer_as_expanded_child */
          child: Container(),
        ),
        Text('Bottom'),
      ],
    );
  }
}

class BadLayout2 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text('Left'),
        Expanded( /* expect: use_spacer_as_expanded_child */
          child: SizedBox(),
        ),
        Text('Right'),
      ],
    );
  }
}

class BadLayout3 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text('A'),
        Expanded( /* expect: use_spacer_as_expanded_child */
          child: Container(color: Colors.transparent),
        ),
        Text('B'),
      ],
    );
  }
}
