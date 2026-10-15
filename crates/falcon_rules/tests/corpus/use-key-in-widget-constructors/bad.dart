import 'package:flutter/widgets.dart';
import 'package:meta/meta.dart';

class MissingConstructor extends StatelessWidget {} /* expect: use-key-in-widget-constructors */

class MissingKey extends StatelessWidget {
  const MissingKey(); /* expect: use-key-in-widget-constructors */
}

class WrongTypedKey extends StatelessWidget {
  const WrongTypedKey({String? key}); /* expect: use-key-in-widget-constructors */
}

