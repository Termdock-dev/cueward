// Read Finder's scripting dictionary without requesting activation or selection.
function failure(error) {
    const number = Number(error.errorNumber || error.number || 0);
    return {code: number === -1743 ? 'permission_denied' : 'unavailable',
        message: 'Finder Apple Events (' + number + '): ' + String(error.message || error)};
}
function itemURL(item) {
    try {
        const url = item.url();
        if (typeof url !== 'string' || url.length === 0) throw new Error('item has no local URL');
        return {status: 'available', url: url};
    } catch (error) { return {status: 'error', error: failure(error)}; }
}
function run(argv) {
    try {
        const limit = Number(argv[0]);
        if (!Number.isInteger(limit) || limit < 1 || limit > 500) {
            return JSON.stringify({Err: {code: 'invalid_options', message: 'max-items must be 1..500'}});
        }
        const finder = Application('com.apple.finder');
        if (!finder.running()) throw new Error('Finder exited before the query');
        const count = finder.finderWindows.length;
        let window = null;
        if (count > 0) {
            const first = finder.finderWindows[0];
            let location;
            try { location = itemURL(first.target()); }
            catch (error) { location = {status: 'error', error: failure(error)}; }
            window = {id: first.id(), location: location};
        }
        const selection = finder.selection();
        if (selection.length > limit) {
            return JSON.stringify({Err: {code: 'scan_limit', message: 'Finder selection exceeds max-items; no partial selection returned'}});
        }
        return JSON.stringify({Ok: {window_count: count, front_window: window,
            selection: selection.map(itemURL)}});
    } catch (error) { return JSON.stringify({Err: failure(error)}); }
}
