var i = 10, j = 0;

while (i > 0) {
    if (i % 2 === 0) {
        j = j + i;
    }

    i--;
}

return j === 30;
