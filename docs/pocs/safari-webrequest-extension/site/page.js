document.querySelector("#probe").addEventListener("click", async () => {
  const response = await fetch(`sample.json?test=${Date.now()}`);
  document.querySelector("#result").textContent = await response.text();
});
