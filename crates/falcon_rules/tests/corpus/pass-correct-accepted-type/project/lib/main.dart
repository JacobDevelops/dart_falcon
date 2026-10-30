import 'api.dart' as api;

void example(dynamic unknown) {
  api.accept(1); /* expect: pass-correct-accepted-type */
  api.animal(value: 'cat'); /* expect: pass-correct-accepted-type */
  api.Box(false); /* expect: pass-correct-accepted-type */
  new api.Box.named(false); /* expect: pass-correct-accepted-type */
  api.Box.shared(1); /* expect: pass-correct-accepted-type */
  api.Box(api.Animal()).send(1); /* expect: pass-correct-accepted-type */
  api.Inherited(api.Animal()).send(1); /* expect: pass-correct-accepted-type */
  api.Box(api.Animal())..send(1); /* expect: pass-correct-accepted-type */
  api.accept('text');
  api.accept(false);
  api.accept(unknown);
  api.animal(value: api.Animal());
  api.animal(value: api.Cat());
  api.animal(value: api.Pet());
  api.Box(api.Cat());
  api.Box.named(api.Pet());
  api.Box.shared('text');
  api.Inherited(api.Cat()).send(false);
}

class Child extends api.Box {
  Child.bad() : super(false); /* expect: pass-correct-accepted-type */
  Child.good() : super(api.Cat());
  Child.redirect(@api.AcceptedTypes({String}) Object value) : super(api.Cat());
  Child.forward() : this.redirect(1); /* expect: pass-correct-accepted-type */
}

void constants() {
  api.constant(1); /* expect: pass-correct-accepted-type */
  api.generic(<int>[1]); /* expect: pass-correct-accepted-type */
  api.constant(false);
  api.generic(<String>['text']);
}
