void handle(bool a, bool b) { if (a) { print(1); } if (b) { print(2); } }
void outer(bool a) { void inner() { if (a) {} } if (a) {} }
void plain() { try { print(1); } finally { print(2); } }
