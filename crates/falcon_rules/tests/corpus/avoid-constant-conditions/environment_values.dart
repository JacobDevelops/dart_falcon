const ordinary = 3;
const environment = bool.fromEnvironment('FLAG');
const derived = !environment;
const present = bool.hasEnvironment('FLAG');
