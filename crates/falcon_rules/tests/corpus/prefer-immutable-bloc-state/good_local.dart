import 'package:bloc/bloc.dart' as real;

class Bloc<E, S> {}
class Cubit<S> {}
class LocalEvent {}
class LocalState {}
class Fake extends Bloc<LocalEvent, LocalState> {}
class FakeCubit extends Cubit<LocalState> {}
