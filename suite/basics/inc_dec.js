var x = 0, a = ++x, b = x++, ok = 0;

if (a === 1) {
    ok = ok + 1;
}

if (b === 1) {
    ok = ok + 1;
}

if (x === 2) {
    ok = ok + 1;
}

ok === 3;
