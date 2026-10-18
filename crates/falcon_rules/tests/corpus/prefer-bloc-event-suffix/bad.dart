import 'package:bloc/bloc.dart';
class Action {} /* expect: prefer-bloc-event-suffix */
class Variant0 extends Action {} /* expect: prefer-bloc-event-suffix */
class Variant1 extends Action {} /* expect: prefer-bloc-event-suffix */
class Variant2 extends Action {} /* expect: prefer-bloc-event-suffix */
class Variant3 extends Action {} /* expect: prefer-bloc-event-suffix */
class Example extends Bloc<Action, int> { Example() : super(0); }
