var data = {
    x: 0,
    get count() {
        var curr = this.x;

        this.x = curr + 1;
        return curr;
    }
};

var i = 0, total = 0;

while (i < 10) {
    total = total + data.count;
    i++;
}

total === 55;
