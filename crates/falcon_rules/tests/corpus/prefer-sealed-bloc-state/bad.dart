import 'package:bloc/bloc.dart';
import 'package:flutter_bloc/flutter_bloc.dart' as fb;
import 'package:meta/meta.dart';

abstract class Snapshot {} /* expect: prefer-sealed-bloc-state */
class First extends Snapshot {} /* expect: prefer-sealed-bloc-state */
class Second extends Snapshot {} /* expect: prefer-sealed-bloc-state */
class Deep extends First {} /* expect: prefer-sealed-bloc-state */
class Other {} /* expect: prefer-sealed-bloc-state */
class Example extends Bloc<int, Snapshot> { Example() : super(First()); }
class Another extends fb.Cubit<Other> { Another() : super(Other()); }

class UnrelatedEvent {}
class UnrelatedState {}
