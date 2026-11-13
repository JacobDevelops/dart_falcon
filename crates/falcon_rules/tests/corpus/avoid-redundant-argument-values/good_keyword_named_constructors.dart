class A { const A.c({bool f = true}); }
void test() { const A.c(f: false); new A.c(f: false); A.c(f: false); }
