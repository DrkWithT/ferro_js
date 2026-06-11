// test void(expr) for ES1+

var test = void(0); // ES1 undefined idiom
var foo = void(1 + 1 + 1);
var ok = 0;

if (test === undefined) {
    ok = ok + 1;
}

if (foo === undefined) {
    ok = ok + 1;
}

ok === 2;
