// Documentation parameters do not become value bindings in a legacy alias.
typedef int Callback(int customer, int customers);

// Signature parameter names must not conflict with names outside the alias.
final customer = 1;
typedef void Handler(int customers);
