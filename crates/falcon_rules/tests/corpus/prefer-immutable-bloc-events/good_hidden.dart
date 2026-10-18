import 'package:bloc/bloc.dart' hide Bloc, Cubit;

class HiddenEvent {}
class HiddenState {}
class Unconfirmed extends Bloc<HiddenEvent, HiddenState> {}
class UnknownCubit extends Cubit<HiddenState> {}
