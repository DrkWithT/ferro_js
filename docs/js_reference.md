### JS References & Objects

#### Objects
 - Collection of properties (keys to values)
 - `this` is a reference to a current object's environment of keys to values.
 - Methods just take an implicit `this` arg or explicit `thisArg` via `Function.prototype.call(thisArg, ...args)`
 - Every object has a hidden `[[prototype]]` that usually provides methods or hidden fields.
   - Functions are objects too, having a for-instance `prototype` that's used when they're called with `new` as ctors.
   - Prototypes _are_ objects!
   - Captured variables can be found via chasing upwards through environments.
   - Closures can be wrapper objects around a function and preserved environment.

#### PropertyDescriptor semantics:
 - Provides a model of any object property with metadata about its usage.
 - 3 types: data (normal), accessor, or generic (an undefined placeholder property)
   - Data:
      - Has `[[writable]], [[configurable]], [[enumerable]]` and `[[value]]`.
      - Example: `obj.foo`
   - Accessor:
      - Has `[[get]], [[set]]` in place of `[[value]]`, excluding `[[writable]]`.
      - `get` and `set` are function references / values.
      - Example: `get foo() {return this.f; }` and `set foo(arg) {this.f = arg;}`
 - Usage:
   - All variable environments _are_ objects with property descriptors.
