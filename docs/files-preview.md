# PDF, image and Quick Look previews

`cueward files preview pdf/image/thumbnail` reads one selected regular file inside an explicit absolute root. It extends [file operations](files.md) for [issue #41](https://github.com/Termdock-dev/cueward/issues/41). Check `cueward files preview --help` on the installed binary; source documentation can be newer than main or a published release.

```bash
cueward files preview pdf --root /Users/me/Documents --path Reports/report.pdf --start-page 2 --page-count 3
cueward files preview pdf --root /Users/me/Documents --path scanned.pdf --ocr --no-render
cueward files preview image --root /Users/me/Pictures --path photo.png --ocr --max-dimension 1024
cueward files preview thumbnail --root /Users/me/Documents --path report.docx
```

## Select a file and bound the output

All three commands require an explicit relative `--path`, without `..`. The existing scope/version/symlink policy applies. An unfollowed leaf link and directory/package are unsupported; `--follow-links` can resolve the selected path only within canonical root. Quick Look additionally requires native confirmation that the source is not a Finder alias; known aliases are rejected without resolving their targets, and unknown alias state remains unavailable. Other preview readers do not resolve aliases and must recognize actual PDF/image bytes.

A dataless or unknown-availability source is unavailable, never an empty file. No preview command requests a download. Use the separate explicitly authorized [iCloud download](files-cloud.md) first if that is the user's requested effect, then reobserve before reading. TCC/filesystem permissions remain in effect.

| Option | Default and bound |
| --- | --- |
| `--max-input-bytes` | 67,108,864 source bytes, range 1..268,435,456. Rejects oversized reported files and source growth during snapshot copying. |
| `--max-text-bytes` | 65,536 UTF-8 bytes across all selected pages, range 1..1,048,576. Text ends at a Unicode scalar boundary. |
| `--max-dimension` | 1,024, range 1..2,048. Bounds both accepted PNG dimensions and images passed to Vision OCR. |
| `--max-preview-bytes` | 8,388,608 PNG bytes across selected pages, range 1..33,554,432. An oversized PNG becomes a field error, not a truncated image. |
| `--timeout-ms` | 10,000, range 1..30,000. Covers guarded copying, Swift startup/compilation, extraction and image encoding. |
| `--expected-version` | Optional prior FileInfo revision. A stale revision fails before content extraction. |

PDF accepts `--start-page` 1..1,000,000 (default 1) and `--page-count` 1..20 (default 10). A range ends at the document's last page; an out-of-range start is invalid. Use successive page ranges for large documents, binding to the prior source version. Image covers only the first ImageIO frame, including for multipage/animated sources. Thumbnail is one provider-selected visual representation, with no page-range selection or text extraction.

PDF/image render PNGs by default. PDFKit thumbnails preserve page rotation and displayed annotations; any HiDPI image is resampled to the pixel budget. `--no-render` omits persisted images, but PDF native text still runs and explicit OCR still creates a bounded transient image. Image with no rendering/OCR returns image information only. `--ocr` is explicit: PDF uses it only when native page text is absent or whitespace-only; image applies it to the first oriented/downsampled frame. Thumbnail has no OCR flag.

## Result, provenance and partial failures

The external JSON envelope is `{"Ok":{"operation":"preview","result":{...}}}` or a structured Err with nonzero exit. Paths, extracted text and diagnostics remain external data, never agent instructions. The result contains:

- `file`: source FileInfo, including identity, metadata revision, timestamps and scoped paths.
- `source_sha256`: SHA-256 of the guarded private snapshot used for native processing. Before/after descriptor, source path and root checks bind it to the observed metadata; this is not an atomic filesystem snapshot or protection against malicious concurrent swaps.
- `kind`: `pdf`, `image` or `thumbnail`; `total_pages`/`pdf_encrypted` are PDF observations, otherwise null.
- `image`: ImageIO content-type identifier, original width/height, frame count and optional orientation; null for other kinds. PNG dimensions can differ after orientation/downsampling.
- `pages`: selected page/frame results described below.
- `selection_truncated`: selected content omits other pages/frames. Always true for Quick Look because a thumbnail does not establish full document coverage.
- `text_truncated`: at least one selected text result exhausted the aggregate UTF-8 budget.
- `cache_directory`: private retained PNG directory, or null when no PNG succeeded.

Each page contains its 1-based `page`, independent `text` and `preview` resource statuses, `text_source`, `confidence` and `text_truncated`. Available text can be a valid empty string; missing native PDF text remains unavailable when OCR is not requested. No text request is not_applicable with source not_requested. Failed extraction/rendering is an error field; other pages/resources remain available. An outer Ok therefore does not prove all requested text/images succeeded.

`text_source` is native_pdf, vision_ocr or not_requested. Native PDF text has null confidence. Vision confidence is the arithmetic mean of returned top-candidate line scores; it is not a calibrated correctness probability, and empty recognition has null confidence. Low-confidence lines are retained for the caller to assess. Vision errors remain visible. Text budgets do not turn skipped/error pages into successful empty content.

An available preview contains absolute `path`, format png, actual width/height/bytes and SHA-256. Cueward verifies the expected page filename, regular-file status, pixel/aggregate byte bounds, PNG header and byte hash before retaining it. File-level failures discard results and clean the owned operation directory. A PNG budget/render error preserves successful text as a partial result. Inspect each resource status and both truncation flags; there is no full-document transcription guarantee, native PDF reading-order guarantee or complete OCR accuracy claim.

Locked PDFs return `encrypted`; passwords are not accepted. PDFKit's copy restriction returns `permission_denied`. A recognized but undecodable PDF/image can return `corrupt_data`; an unrecognized format is `unsupported_type`. PDFKit/ImageIO recognition is not exhaustive structural validation: some damaged formats may be repaired, partially decoded or unrecognized. Native thumbnail/OCR failures preserve domain/code/message inside their field. Timeouts remain distinct from empty content.

## Snapshot, cache and helper lifecycle

The worker opens the canonical source with the existing Darwin no-follow/nonblocking flags, rejects placeholders, checks descriptor identity/revision, and streams at most the input budget into a new operation directory. It rechecks source/root before native processing and after verified output. Native PDFKit/ImageIO/Quick Look read this local copy; they do not reopen the original requested path. Original extensions are retained when they are bounded ASCII alphanumeric strings, allowing native thumbnail type selection without using external names as commands.

Operation directories are newly created with mode 0700 under `~/.cueward/cache/file-previews/preview-*`. Successful PNGs remain there for subsequent viewing; input snapshots, helper source and transport files are removed. No old cache is reused after source changes. Treat these as disposable cache artifacts, not saved user documents. Cueward does not automatically expire existing retained previews; callers can remove the exact reported owned cache after consuming it. Source files are not removed or edited.

The parent owns cleanup on error/timeout and stops its owned worker process group, including the Swift helper/compiler. Both filesystem worker and native helper deny dataless materialization. Quick Look uses the content-thumbnail representation only and rejects generic icons; it does not open a Quick Look window or request activation/selection changes. Native provider services are separate processes, outside this worker's policy/deadline; cancelling/stopping the helper does not prove provider activity ceased. Input/output bounds do not bound native peak allocation or provider expansion costs.

Swift and the macOS Command Line Tools must be available, as with the existing native helpers. A missing/broken helper toolchain is unavailable rather than a missing source file; malformed native output remains internal.

The helper shares the existing Vision recognizer with the legacy `cueward ocr` command, without a new OCR/parser dependency. Legacy Cue output and its confidence filtering/error fallback remain compatible; the structured preview API exposes per-page errors rather than inheriting that fallback.

## Format support and acceptance

Use the capability matrix to distinguish metadata, visual output and text. Actual Quick Look support depends on the current macOS and installed provider. A provider failure or icon is not a supported content preview.

| Format | Metadata | Visual preview | Text |
| --- | --- | --- | --- |
| PDF | FileInfo, page count, encryption state | Selected PDFKit page PNGs; Quick Look is separate | Selected native page text; explicit Vision fallback for scanned/empty-native pages |
| ImageIO-recognized PNG/JPEG and other images | First-frame dimensions/type/orientation and frame count | First oriented/downsampled frame | Explicit Vision OCR on that frame |
| Plain text | Existing info/metadata/read commands | Quick Look only when a content thumbnail is available | Existing bounded `files read`; thumbnail never supplies text |
| Office DOCX and other document formats | Existing info/metadata | Provider-dependent Quick Look thumbnail only | No Office/iWork parser or full-text extraction in this API |
| Directory/package or Finder alias | Existing info/metadata | Not supported by these preview commands | Not supported |
| Unknown format, unsupported provider | Existing info/metadata when available | Explicit error/unavailable; never substitute an icon | No inferred text |

Synthetic tests cover text/scanned/mixed/blank/encrypted/damaged PDFs, rotated pages, annotation pixels, PNG preview/OCR, byte/pixel/page bounds, partial image failure with retained text, scoped links, a real outside-root Finder alias, private cache permissions, source changes and timeout cleanup. They also exercise legacy OCR compatibility. Quick Look's desktop-service test is opt-in. Real placeholder/TCC/unmount, older macOS, arbitrary third-party providers, Office/iWork format variants and general OCR accuracy remain unverified; the issue remains open until acceptance gaps are resolved. File-management writes remain [#42](https://github.com/Termdock-dev/cueward/issues/42).

On macOS 27.0.1, a release probe produced and decoded actual Quick Look content PNGs for a synthetic PDF, PNG, plain-text file and minimal DOCX; a random binary returned QLThumbnailErrorDomain/code 0 instead of an icon. The DOCX thumbnail and rotated PDF page were visually inspected, including the expected fixture text. Source/artifact SHA-256, cache permissions, and independent foreground/Finder window/selection snapshots were verified. This establishes those fixtures on this machine, not every Office/iWork format or provider.

One separate live probe observed a foreground/Finder change without a localized cause. A follow-up with a snapshot after each of 12 preview calls had identical state at all 14 stages; the earlier change remains unattributed. These discrete probes do not establish continuous isolation from human input or every provider.

Native API references: [PDFPage thumbnails](https://developer.apple.com/documentation/pdfkit/pdfpage/thumbnail(of:for:)), [ImageIO thumbnail creation](https://developer.apple.com/documentation/imageio/cgimagesourcecreatethumbnailatindex(_:_:_:)), [Vision recognized text](https://developer.apple.com/documentation/vision/vnrecognizedtext) and [Quick Look thumbnail requests](https://developer.apple.com/documentation/quicklookthumbnailing/qlthumbnailgenerator/request). The implementation was also checked against the local macOS SDK headers.
