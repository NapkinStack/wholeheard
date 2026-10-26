// wholeheard-speech — Apple's SpeechAnalyzer, run by wholeheard-transcription (D42, ADR-0012).
//
//     wholeheard-speech status <locale>                what this Mac offers for the locale
//     wholeheard-speech install <locale>               Apple's model of it, fetched by macOS
//     wholeheard-speech transcribe <file> <locale>     one JSON line a segment: start, end, text
//
// Nothing is fetched but by `install`, which Wholeheard runs only on the user's click. What it
// prints is the meeting's text, for the program that runs it alone; its errors say no word of it.

import AVFoundation
import Foundation
import Speech

struct Refusal: Error {
    let sentence: String
}

@main
struct WholeheardSpeech {
    static func main() async {
        let arguments = Array(CommandLine.arguments.dropFirst())
        do {
            switch (arguments.first, arguments.count) {
            case ("status", 2): try await status(Locale(identifier: arguments[1]))
            case ("install", 2): try await install(Locale(identifier: arguments[1]))
            case ("transcribe", 3): try await transcribe(URL(fileURLWithPath: arguments[1]), Locale(identifier: arguments[2]))
            default:
                throw Refusal(sentence: "usage: wholeheard-speech status <locale> | install <locale> | transcribe <file> <locale>")
            }
        } catch let refusal as Refusal {
            fail(refusal.sentence)
        } catch {
            fail("SpeechAnalyzer failed: \(error.localizedDescription)")
        }
    }

    /// Whether SpeechTranscriber runs on this Mac, knows the locale, and has its model here.
    static func status(_ locale: Locale) async throws {
        let wanted = locale.identifier(.bcp47)
        let supported = await SpeechTranscriber.supportedLocales.contains { $0.identifier(.bcp47) == wanted }
        let installed = await SpeechTranscriber.installedLocales.contains { $0.identifier(.bcp47) == wanted }
        say(["available": SpeechTranscriber.isAvailable, "supported": supported, "installed": installed])
    }

    /// Apple's model of the locale, fetched and installed by macOS — on the user's word only.
    static func install(_ locale: Locale) async throws {
        let transcriber = try await transcriberOf(locale, timed: false)
        if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
            try await request.downloadAndInstall()
        }
        say(["installed": true])
    }

    /// The file heard whole, in the locale: one line a result, its time from its words'.
    static func transcribe(_ file: URL, _ locale: Locale) async throws {
        let transcriber = try await transcriberOf(locale, timed: true)
        let installed = await SpeechTranscriber.installedLocales.contains { $0.identifier(.bcp47) == locale.identifier(.bcp47) }
        guard installed else {
            throw Refusal(sentence: "Apple's model of \(locale.identifier(.bcp47)) is not on this Mac")
        }
        let analyzer = SpeechAnalyzer(modules: [transcriber])
        let audio = try AVAudioFile(forReading: file)
        try await analyzer.start(inputAudioFile: audio, finishAfterFile: true)
        for try await result in transcriber.results {
            let times = result.text.runs.compactMap(\.audioTimeRange)
            let text = String(result.text.characters).trimmingCharacters(in: .whitespacesAndNewlines)
            guard let first = times.first, let last = times.last, !text.isEmpty else { continue }
            say(["start": first.start.seconds, "end": last.end.seconds, "text": text])
        }
    }

    /// A transcriber of the locale, reserved for this program as Yap reserves it: the system keeps
    /// a few locales per program, so the others it held are given back first.
    static func transcriberOf(_ locale: Locale, timed: Bool) async throws -> SpeechTranscriber {
        guard SpeechTranscriber.isAvailable else {
            throw Refusal(sentence: "SpeechTranscriber is not available on this Mac")
        }
        let wanted = locale.identifier(.bcp47)
        guard await SpeechTranscriber.supportedLocales.contains(where: { $0.identifier(.bcp47) == wanted }) else {
            throw Refusal(sentence: "SpeechTranscriber does not hear \(wanted)")
        }
        for reserved in await AssetInventory.reservedLocales where reserved.identifier(.bcp47) != wanted {
            await AssetInventory.release(reservedLocale: reserved)
        }
        try await AssetInventory.reserve(locale: locale)
        return SpeechTranscriber(
            locale: locale,
            transcriptionOptions: [],
            reportingOptions: [],
            attributeOptions: timed ? [.audioTimeRange] : []
        )
    }

    static func say(_ line: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: line, options: [.sortedKeys]) else { return }
        FileHandle.standardOutput.write(data + Data("\n".utf8))
    }

    static func fail(_ sentence: String) -> Never {
        FileHandle.standardError.write(Data("wholeheard-speech: \(sentence)\n".utf8))
        exit(1)
    }
}
