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

/// Good: cleanup with super at end
class ScrollDispose extends ChangeNotifier {
  late ScrollController scrollController;

  @override
  void dispose() {
    scrollController.dispose();
    super.dispose();
  }
}

/// Good: multiple cleanup steps then super
class ComplexDispose extends ChangeNotifier {
  late PageController pageController;
  late TabController tabController;
  late StreamSubscription streamSub;

  @override
  void dispose() {
    pageController.dispose();
    tabController.dispose();
    streamSub.cancel();
    super.dispose();
  }
}
