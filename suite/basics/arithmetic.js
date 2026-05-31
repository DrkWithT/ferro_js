// test simple arithmetic

var ok = 0;

var a = (3 * 10) + (37 - 1) / 3;

if (a !== 42) {
    return false;
}

var b = 15 & 3, c = 16 | 3;//, d = 1 << 4, e = 16 >> 4;

if (b === 3) {
    ok++;
}

if (c === 19) {
    ok++;
}

return ok === 4;
