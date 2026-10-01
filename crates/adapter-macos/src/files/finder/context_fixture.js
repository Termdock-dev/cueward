// Replace only the Finder Apple Events boundary; execute production query code.
function appleError(number) {
    const error = new Error('fixture native error');
    error.errorNumber = number;
    return error;
}
var fixtureURLReads = 0;
function fixtureItem(value) {
    return {url: function () {
        fixtureURLReads += 1;
        if (value.error) throw appleError(value.error);
        return value.url;
    }};
}
function fixtureApplication(id) {
    if (id !== 'com.apple.finder') throw new Error('unexpected recipient');
    const windows = fixture.windows || 0;
    return {
        running: function () { return true; },
        get finderWindows() {
            if (fixture.permission) throw appleError(-1743);
            return windows ? [{id: function () { return fixture.id; },
                target: function () { return fixtureItem(fixture.location); }}] : [];
        },
        selection: function () { return (fixture.selection || []).map(fixtureItem); }
    };
}
