const filter = { urls: ["http://127.0.0.1/*"] };
let pendingWrite = Promise.resolve();

function record(phase, details) {
  const entry = {
    phase,
    url: details.url,
    method: details.method ?? null,
    status: details.statusCode ?? null,
    type: details.type,
    time: details.timeStamp,
  };

  pendingWrite = pendingWrite.then(async () => {
    const { events = [] } = await browser.storage.local.get("events");
    await browser.storage.local.set({ events: [...events, entry].slice(-50) });
  }).catch((error) => console.error("Cueward WebRequest PoC:", error));
}

browser.webRequest.onBeforeRequest.addListener(
  (details) => record("before", details),
  filter,
);
browser.webRequest.onCompleted.addListener(
  (details) => record("complete", details),
  filter,
);
browser.webRequest.onErrorOccurred.addListener(
  (details) => record("error", details),
  filter,
);
