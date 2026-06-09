import 'package:flutter/material.dart';

/// Cleanup before calling super.dispose
class GoodDisposeOrder extends ChangeNotifier {
  late AnimationController controller;
  late StreamSubscription subscription;

  @override
  void dispose() {
    controller.dispose();
    subscription.cancel();
    super.dispose();
  }
}

/// Another good example
class AnotherGoodDispose extends ChangeNotifier {
  late TextEditingController textController;
  late FocusNode focusNode;

  @override
  void dispose() {
    textController.dispose();
    focusNode.dispose();
    super.dispose();
  }
}

/// Multiple resources with correct order
class MultipleResourceDispose extends ChangeNotifier {
  late StreamController streamController;
  late Timer timer;

  @override
  void dispose() {
    streamController.close();
    timer.cancel();
    super.dispose();
  }
}

/// Single resource cleanup
class SingleResourceDispose extends ChangeNotifier {
  late AnimationController animation;

  @override
  void dispose() {
    animation.dispose();
    super.dispose();
  }
}
