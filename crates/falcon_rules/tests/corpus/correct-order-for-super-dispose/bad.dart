import 'package:flutter/material.dart';

/// Calling super.dispose before cleanup
class BadDisposeOrder extends ChangeNotifier {
  late AnimationController controller;
  late StreamSubscription subscription;

  @override
  void dispose() {
    super.dispose(); /* expect: correct-order-for-super-dispose */
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
    super.dispose(); /* expect: correct-order-for-super-dispose */
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
    super.dispose(); /* expect: correct-order-for-super-dispose */
    streamController.close();
    timer.cancel();
  }
}

/// Bad: super.dispose in middle
class MiddleDispose extends ChangeNotifier {
  late ScrollController scrollController;

  @override
  void dispose() {
    scrollController.dispose();
    super.dispose(); /* expect: correct-order-for-super-dispose */
    print('cleanup done');
  }
}

/// Bad: early super call with multiple cleanups
class EarlySuper extends ChangeNotifier {
  late PageController pageController;
  late TabController tabController;

  @override
  void dispose() {
    super.dispose(); /* expect: correct-order-for-super-dispose */
    pageController.dispose();
    tabController.dispose();
  }
}
