import 'package:bloc/bloc.dart';
class ActionEvent {}
class Variant0Event extends ActionEvent {}
class Variant1Event extends ActionEvent {}
class Variant2Event extends ActionEvent {}
class Variant3Event extends ActionEvent {}
class Example extends Bloc<ActionEvent, int> { Example() : super(0); }
class Unrelated {}
