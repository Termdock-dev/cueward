# Data Flow Architecture (The Cueward Advantage)

## 1. The Standard Agent Model (Cloud-heavy, Blackbox)
When a typical CLI Agent (like the default Gemini CLI) interacts with a user's local files (e.g., a PDF, an image, or a database), the process looks like this:

1. **Extraction (Dumb)**: The agent uses a generic tool (e.g., `read_file`) to grab the raw binary data or the entire text block of the file.
2. **Transmission (Heavy)**: The entire file (or massive chunks of it) is sent over the network, appended to the prompt, to the cloud LLM backend.
3. **Processing (Blackbox)**: The cloud server processes the multimodal data (images, PDFs) using its massive neural networks. This step is opaque to the user.
4. **Output (Lossy)**: Because of context window limits, if the data is too large, the LLM performs "lossy summarization" (skipping details, extracting only high-level points) to fit the prompt limits. 

**Problems**: Extremely high token usage, slow latency, massive privacy risks (sending personal data to the cloud), and loss of granular control.

## 2. The Cueward Model (Edge-compute, Transparent)
Cueward intercepts the data *before* it ever reaches the cloud LLM. It acts as an intelligent local funnel.

1. **Extraction (Smart/Native)**: Cueward uses macOS native APIs (PDFKit, Vision OCR, SQLite, AppleScript). It knows *how* to extract the exact text it needs. It doesn't send a PDF image; it uses local PDFKit to instantly pull the string layer.
2. **Pre-processing (Local Edge)**: If a PDF has 100 pages, Cueward can filter out pages with no text, remove boilerplate headers/footers, and compress the data into a clean, lightweight JSON structure (`Cue`).
3. **Transmission (Lightweight)**: Cueward sends *only the synthesized text prompt* (the `Cue` object) to the LLM for high-level reasoning (e.g., "Summarize the key action items from this text").
4. **Output (Precise)**: The LLM returns a structured JSON response (e.g., `{"tasks": ["Call client"], "tags": ["work"]}`).
5. **Action (Local Execution)**: Cueward receives the JSON and uses local AppleScript to insert the task into the user's Reminders or Calendar.

**Advantages**: Near-zero token waste, lightning-fast execution, complete privacy control (data is pre-processed locally), and no blackbox "skipping" of large files because the heavy lifting (OCR/text extraction) is done by the Mac's local hardware.
