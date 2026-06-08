### DerkJS VM

#### Basics
 - Stack-based value passing
    - Temporary stack
    - Environments stack / chain
    - Completion record stack
 - TCO dispatch with hot data passed directly to opcode handlers
 - In-place value ops / super-instructions?
 - Mark & sweep GC

#### Call Layout on Stack:
```
EXAMPLE: call of foo(1, 2):
-----------------
TEMP STACK, pre-call:
| ......... | <-- UNFILLED ARGS become UNDEFINED!
| Value(2)  | <-- STACK[SP]
| Value(1)  |
| ref?this  | <-- STACK[CALLEE_BP + 1] is thisArg, defaulted to globalThis.
| ref(foo)  | <-- STACK[CALLEE_BP] uses CALLEE_BP = SP - (ARGC + 1)
-----------------
Begin call:
1. Load array-like object viewing N temp args to Environment.arguments. A new environment is created if:
   - Case 1: Global scope / code is first entered.
   - Case 2: Constructor calls make a new environment object for `this`.
   - Case 3: If a function captures foreign names or has captured names.
      - Functions are checked at compile time for "foreign" captured names... If there's none, the function is marked "plain", not requiring an environment object itself (but a passed closure argument will!)
      - Bind `this` to global environment IFF the call is regular.
-------...-------
End call:
1. Pop environment IFF there's no function returned, leaving a completion record with the return value. Otherwise, a closure is returned in the record.
2. The destination of the callee's result gets the unpacked [[value]].
```

#### Normal Opcodes
 - PUSH_UNDEF
 - PUSH_NULL
 - PUSH_BOOL
 - PUSH_NAN
 - PUSH_INF
 - PUSH_NEG_INF
 - PUSH_CONST
 - DUP1        NOTE: a -> a a
 - DUP2        NOTE: a b -> a a b
 - SWAP        NOTE: a b -> b a
 - GET_LOCAL   NOTE: only used if no environment object is needed.
 - SET_LOCAL
 - GET_VAR     NOTE: tries getting a var from the callee's environment object, etc.
 - SET_VAR
 - MAKE_OBJ
 - GET_OWN_PROP
 - SET_OWN_PROP
 - GET_PROP
 - SET_PROP
 - DEL_PROP
 - GET_PROTO
 - SET_PROTO
 - TO_BOOLEAN
 - TO_NUMBER
 - NEGATE_BOOL
 - NEGATE_NUM
 - MOD
 - MUL
 - DIV
 - ADD
 - SUB
 - JUMP_EQ
 - JUMP_NE
 - JUMP_LT
 - JUMP_LTE
 - JUMP_GT
 - JUMP_GTE
 - JUMP_IF
 - JUMP_ELSE
 - JUMP
 - CALL
 - CALL_CTOR
 - RET
 - RET_CLOSURE ?

#### Super Opcodes ?
 - MUL_K
 - DIV_K
 - ADD_K
 - SUB_K
 - 
