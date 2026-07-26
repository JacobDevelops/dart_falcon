import 'package:flutter/widgets.dart';
import 'package:meta/meta.dart';

class MissingConstructor extends StatelessWidget {} /* expect: use-key-in-widget-constructors */

class MissingKey extends StatelessWidget {
  const MissingKey(); /* expect: use-key-in-widget-constructors */
}

class WrongTypedKey extends StatelessWidget {
  const WrongTypedKey({String? key}); /* expect: use-key-in-widget-constructors */
}

class WrongForwardingType extends StatelessWidget {
  const WrongForwardingType({required String value}) : super(key: value); /* expect: use-key-in-widget-constructors */
}

@visibleForTesting
class TestWidget extends StatelessWidget {} /* expect: use-key-in-widget-constructors */
