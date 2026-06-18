// test primitive strings

var s1 = 'foo', s2 = "bar", s3 = "ABC", s4 = '\x41\x42\x43', ok = 0;

if (s1 !== s2) {
    ok++;
}

if (s3 === s4) {
    ok++;
}

// if (s3.length !== 3) {
//     return false;
// }

return ok;
