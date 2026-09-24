const output = document.querySelector("#events");

async function refresh() {
  const { events = [] } = await browser.storage.local.get("events");
  output.textContent = events.length
    ? events.map(({ phase, method, status, type, url }) =>
        `${phase} ${method ?? ""} ${status ?? ""} ${type ?? ""} ${url}`
      ).join("\n")
    : "No requests recorded. Open the local test page and click Fetch sample JSON.";
}

document.querySelector("#refresh").addEventListener("click", refresh);
document.querySelector("#clear").addEventListener("click", async () => {
  await browser.storage.local.set({ events: [] });
  await refresh();
});

refresh().catch((error) => { output.textContent = String(error); });
