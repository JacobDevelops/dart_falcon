import 'package:bloc/bloc.dart';

class Unrelated {}
class AlsoUnrelated extends Unrelated {}
class Example extends Cubit<Object> { Example() : super(0); }
