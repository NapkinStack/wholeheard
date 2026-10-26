// swift-tools-version: 6.1
// wholeheard-speech: Apple's SpeechAnalyzer for wholeheard-transcription, on a Mac with macOS 26
// (D42, ADR-0012). No dependency: the system's Speech and AVFoundation alone.

import PackageDescription

let package = Package(
    name: "wholeheard-speech",
    platforms: [.macOS("26")],
    targets: [
        .executableTarget(name: "wholeheard-speech", path: "Sources/wholeheard-speech")
    ]
)
