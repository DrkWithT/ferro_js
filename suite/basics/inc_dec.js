var x = 0, a = ++x, b = x++, c = 4, d = --c, e = c--, ok = 0;

if (a === 1) {
    ok = ok + 1;
}

if (b === 1) {
    ok = ok + 1;
}

if (x === 2) {
    ok = ok + 1;
}

if (d === 3) {
    ok = ok + 1;
}

if (e === 3) {
    ok = ok + 1;
}

ok === 5;
