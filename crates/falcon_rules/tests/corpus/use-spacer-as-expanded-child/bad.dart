import 'package:flutter/material.dart';

// Bad: Expanded with empty Container or SizedBox

class BadLayout1 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Text('Top'),
        Expanded( /* expect: use-spacer-as-expanded-child */
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
        Expanded( /* expect: use-spacer-as-expanded-child */
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
        Expanded( /* expect: use-spacer-as-expanded-child */
          child: Container(color: Colors.transparent),
        ),
        Text('B'),
      ],
    );
  }
}

class BadLayout4 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Text('Header'),
        Expanded( /* expect: use-spacer-as-expanded-child */
          child: SizedBox(width: double.infinity),
        ),
        Text('Footer'),
      ],
    );
  }
}

class BadLayout5 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Icon(Icons.arrow_left),
        Expanded( /* expect: use-spacer-as-expanded-child */
          child: Container(decoration: BoxDecoration()),
        ),
        Icon(Icons.arrow_right),
      ],
    );
  }
}
