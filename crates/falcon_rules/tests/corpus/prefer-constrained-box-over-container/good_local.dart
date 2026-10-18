class Container {
  Container({Object? constraints, Object? child});
  const Container.named({Object? constraints, Object? child});
}

void unrelated(Object value) {
  Container(constraints: value, child: value);
  new Container(constraints: value, child: value);
  const Container.named(constraints: 1, child: 2);
}
