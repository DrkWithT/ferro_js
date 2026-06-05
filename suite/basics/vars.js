// test filled vars
var a = 1, b, ok = 0;

if (a === 1) {
    ok = ok + 1;
}

if (b === undefined) {
    ok = ok + 1;
}

return ok === 2;
