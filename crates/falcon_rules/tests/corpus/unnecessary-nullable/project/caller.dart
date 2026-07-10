import 'service.dart';

void driver() {
  final s = Service();
  s._never(1);
  s._never(2);
  s._sometimes(1);
  s._sometimes(null); // makes _sometimes genuinely nullable → not flagged
  s._assigns('x');
  s._ambiguous(3);
  _topNever('hello');
  Other()._ambiguous(4);
}
