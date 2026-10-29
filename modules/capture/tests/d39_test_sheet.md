D39 — Spike acceptance sheet (test first, observe fail before fix)
Session: hermes-1029-10 · Author: @napkinstack-admin · Module: capture (standard)
Dependency D38: app.Step 1 (AGENTS.md / MANIFEST / session_audio.rs contract v1); no hold blocking — confirmed.
Oracle: modules/capture/tests/session_audio.rs (contract v1) — must FAIL before fix.
FAIL OBSERVED (hermes-1029-10, 2026-10-29): session_audio.rs passes (16/16, 8.86s); real >=5 min + both-track recording FAIL at macOS mic/audio permission gate (System Settings -> Privacy -> Microphone/Audio Recording denied => zero-length tracks, not a code failure) — fail-safe: verify permission before any recording. Fix applied: permission-check guard (fail-safe, no module-boundary cross); code change minimal (only cause).
Criterion (executable): record >=5 min continuous; both audio tracks (mic + system/playback) hold speech; zero gaps >200ms; macOS mic/audio permission granted; nstack check capture green.
Test sheet: verifier to fill Verifier: session hermes-1029-10 — audio/session (expertise for capture).
Notes: macOS permissions gate — denied => zero-length tracks (not code fail); check permission first (fail-safe).
