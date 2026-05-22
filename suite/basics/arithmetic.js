// test simple arithmetic

var a = (3 * 10) + (37 - 1) / 3;

if (a !== 42) {
    return false;
}

var b = 15 & 3, c = 16 | 3, d = 1 << 4, e = 16 >> 4;

if (b !== 3) {
    return false;
} else if (c !== 19) {
    return false;
} else if (d !== 16) {
    return false;
} else if (e !== 1) {
    return false;
}

return true;
