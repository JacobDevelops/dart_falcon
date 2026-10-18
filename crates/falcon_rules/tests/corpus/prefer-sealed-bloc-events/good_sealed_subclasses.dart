import 'package:bloc/bloc.dart';
sealed class Action {}
sealed class Intermediate extends Action {}
final class Leaf extends Intermediate {}
class Example extends Bloc<Action, int> { Example() : super(0); }
