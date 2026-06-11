import 'package:flutter/material.dart';

// Direct construction of banned widgets must be flagged when the rule is
// configured (see config.json in this directory).

Widget buildPage(BuildContext context) {
  return Container( /* expect: use-design-system-item */
    child: Text('hello'), /* expect: use-design-system-item */
  );
}

Widget buildButton() {
  return ElevatedButton( /* expect: use-design-system-item */
    onPressed: null,
    child: null,
  );
}

class HomePage {
  Widget make() {
    return new Container(); /* expect: use-design-system-item */
  }

  Widget scaffold() {
    return Scaffold(); /* expect: use-design-system-item */
  }
}
