import 'package:bloc/bloc.dart';
import 'package:meta/meta.dart';

@immutable
sealed class Snapshot {}
@immutable
final class First extends Snapshot {}
@immutable
final class Second extends Snapshot {}
class Example extends Bloc<int, Snapshot> { Example() : super(First()); }
@immutable
final class Other {}
class Another extends Cubit<Other> { Another() : super(Other()); }

class UnrelatedEvent {}
class UnrelatedState {}
class NoSuffix {}
