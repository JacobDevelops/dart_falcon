import 'package:flutter/material.dart';

/// Calling super.dispose before cleanup
class BadDisposeOrder extends ChangeNotifier {
  late AnimationController controller;
  late StreamSubscription subscription;

  @override
  void dispose() {
    super.dispose(); /* expect: correct_order_for_super_dispose */
    controller.dispose();
    subscription.cancel();
  }
}

/// Another bad example
class AnotherBadDispose extends ChangeNotifier {
  late TextEditingController textController;
  late FocusNode focusNode;

  @override
  void dispose() {
    super.dispose(); /* expect: correct_order_for_super_dispose */
    textController.dispose();
    focusNode.dispose();
  }
}

/// Multiple resources with super called first
class MultipleResourceDispose extends ChangeNotifier {
  late StreamController streamController;
  late Timer timer;

  @override
  void dispose() {
    super.dispose(); /* expect: correct_order_for_super_dispose */
    streamController.close();
    timer.cancel();
  }
}
