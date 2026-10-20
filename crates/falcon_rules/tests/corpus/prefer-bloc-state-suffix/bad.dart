import 'package:bloc/bloc.dart';
class Action {} /* expect: prefer-bloc-state-suffix */
class Variant0 extends Action {} /* expect: prefer-bloc-state-suffix */
class Variant1 extends Action {} /* expect: prefer-bloc-state-suffix */
class Variant2 extends Action {} /* expect: prefer-bloc-state-suffix */
class Variant3 extends Action {} /* expect: prefer-bloc-state-suffix */
class Example extends Bloc<int, Action> { Example() : super(Action()); }
