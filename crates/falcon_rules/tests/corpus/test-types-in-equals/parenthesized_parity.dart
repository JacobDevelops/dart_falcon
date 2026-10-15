// Analyzer reports no lint for adv-AdvB4/repro/lib/ttie_paren.dart.
class D {
  @override
  bool operator ==(Object other) {
    final x = (other) as D;
    return x.hashCode == 1;
  }
  @override
  int get hashCode => 0;
}
