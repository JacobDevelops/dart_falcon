// @dart = 3.13

import 'package:flutter/widgets.dart';

abstract class MyWidget({super.key}) extends StatelessWidget {}

abstract class TestWidget() extends StatefulWidget {
  @visibleForTesting
  this;
}
