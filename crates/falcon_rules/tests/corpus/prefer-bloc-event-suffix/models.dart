import 'package:bloc/bloc.dart';
class ImportedModel {}
abstract class ImportedBase<E, S> extends Bloc<E, S> { ImportedBase(S state) : super(state); }
