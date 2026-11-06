import AVFoundation
import Foundation
import Speech

// Usage: swift voice_memo_transcribe.swift <audio-path> [locale]
// Every outcome the helper can name is one JSON object on stdout with exit status 0.
// A non-zero exit means the helper did not run to an outcome.

// All-zero audio makes the model emit one token near 0.07 confidence. Synthetic speech at
// -10 dB signal-to-noise still had a token at 0.7, so the floor sits between the two.
let minimumPeakConfidence = 0.2

struct HelperResult: Encodable {
    var status: String
    var locale: String?
    var text: String?
    var installed: [String]?
    var message: String?
}

struct LocaleRefusal: Error {
    let result: HelperResult
}

func installedNames() async -> [String] {
    await SpeechTranscriber.installedLocales.map(\.identifier).sorted()
}

// Checked before any AssetInventory call so that a missing locale never starts asset work.
func installedLocale(matching requested: Locale) async throws -> Locale {
    guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: requested) else {
        throw LocaleRefusal(result: HelperResult(
            status: "unsupported_locale",
            locale: requested.identifier,
            installed: await installedNames()
        ))
    }
    guard await SpeechTranscriber.installedLocales.contains(locale) else {
        throw LocaleRefusal(result: HelperResult(
            status: "locale_not_installed",
            locale: locale.identifier,
            installed: await installedNames()
        ))
    }
    return locale
}

func transcribe(path: String, requested: Locale) async throws -> HelperResult {
    guard let file = try? AVAudioFile(forReading: URL(fileURLWithPath: path)) else {
        return HelperResult(status: "unreadable_audio")
    }
    let locale = try await installedLocale(matching: requested)
    let transcriber = SpeechTranscriber(
        locale: locale,
        transcriptionOptions: [],
        reportingOptions: [],
        attributeOptions: [.transcriptionConfidence]
    )
    if try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) != nil {
        throw LocaleRefusal(result: HelperResult(
            status: "locale_not_installed",
            locale: locale.identifier,
            installed: await installedNames()
        ))
    }
    let analyzer = SpeechAnalyzer(modules: [transcriber])
    let reader = Task { () -> (text: String, peakConfidence: Double?) in
        var text = ""
        var peakConfidence: Double?
        for try await result in transcriber.results {
            text += String(result.text.characters)
            for run in result.text.runs {
                if let confidence = run.transcriptionConfidence {
                    peakConfidence = max(peakConfidence ?? confidence, confidence)
                }
            }
        }
        return (text, peakConfidence)
    }
    if let last = try await analyzer.analyzeSequence(from: file) {
        try await analyzer.finalizeAndFinish(through: last)
    } else {
        await analyzer.cancelAndFinishNow()
    }
    let (rawText, peakConfidence) = try await reader.value
    let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
    if text.isEmpty || (peakConfidence ?? 1) < minimumPeakConfidence {
        return HelperResult(status: "no_speech", locale: locale.identifier)
    }
    return HelperResult(status: "transcribed", locale: locale.identifier, text: text)
}

func emit(_ result: HelperResult) {
    guard let data = try? JSONEncoder().encode(result) else {
        fputs("failed to encode helper result\n", stderr)
        exit(1)
    }
    print(String(decoding: data, as: UTF8.self))
}

let semaphore = DispatchSemaphore(value: 0)
Task {
    let arguments = CommandLine.arguments
    let requested = arguments.count > 2 ? Locale(identifier: arguments[2]) : Locale.current
    do {
        emit(try await transcribe(path: arguments[1], requested: requested))
    } catch let refusal as LocaleRefusal {
        emit(refusal.result)
    } catch {
        emit(HelperResult(status: "error", message: "\(error)"))
    }
    semaphore.signal()
}
semaphore.wait()
