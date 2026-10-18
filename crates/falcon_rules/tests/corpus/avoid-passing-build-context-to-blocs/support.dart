import 'package:bloc/bloc.dart';
import 'package:flutter/widgets.dart';

class Counter extends Cubit<int> {
  Counter() : super(0);
}
class Consumer extends Bloc<Object, int> {
  Consumer(Object dependency) : super(0);
  Consumer.named({required Object dependency}) : super(0);
  void accept(Object dependency) {}
}
class ChildConsumer extends Consumer {
  ChildConsumer(Object dependency) : super(dependency);
}
class View extends StatelessWidget {
  View(Object dependency);
  View.named({required Object dependency});
}
class Repository {
  Repository(Object dependency);
  void accept(Object dependency) {}
}
