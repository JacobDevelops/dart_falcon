class Parent { Parent(); }
class Child extends Parent {
  Child() : /* expect: avoid-unnecessary-super */ super();
}
