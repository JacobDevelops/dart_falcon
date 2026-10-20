import 'package:equatable/equatable.dart';
import 'package:equatable/equatable.dart' as eq;
class FirstState extends Equatable {}
class SecondState with EquatableMixin {}
class ThirdState extends FirstState {}
class FourthState extends eq.Equatable {}
class FifthState with eq.EquatableMixin {}
class NotConfigured {}
class UnknownState extends Missing {}
class InterfaceState implements Equatable {}
mixin class ViewState {}
