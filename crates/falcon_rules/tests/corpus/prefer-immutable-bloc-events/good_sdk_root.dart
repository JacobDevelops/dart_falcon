import 'package:bloc/bloc.dart';

class Unrelated {}
class AlsoUnrelated extends Unrelated {}
class Example extends Bloc<Object, int> { Example() : super(0); }
