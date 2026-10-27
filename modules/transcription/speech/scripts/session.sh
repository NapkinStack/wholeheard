#!/usr/bin/env bash
# On a Mac: a session as capture writes it, heard by SpeechAnalyzer through the step, as the app
# hears it — at a voice's level and a hundred times quieter (#426: a Mac's session heard as
# nothing). Fails when the session at a voice's level gives no segment. Run from the repository.
set -euo pipefail
bin=modules/transcription/target/release
here=modules/transcription/speech
cp "$here/.build/release/wholeheard-speech" "$bin/"
if ! "$bin/wholeheard-speech" status fr-FR | grep -q '"installed":true'; then
  echo "::warning::not run — this runner's SpeechTranscriber has no fr-FR model installed"; exit 0
fi
voice=$(say -v '?' | awk '/fr_FR/ {print $1; exit}')
[ -n "$voice" ] || { echo "::warning::not run — no French voice"; exit 0; }
text="Bonjour à tous, merci d'être là. La réunion commence maintenant. Nous allons parler du budget \
de l'année prochaine, puis du recrutement de deux personnes dans l'équipe technique. Ensuite Karim \
présentera le calendrier du projet Atlas. Enfin nous ferons le point sur les actions de la semaine."
work=$(mktemp -d)
say -v "$voice" -o "$work/spoken.aiff" "$text $text $text"
afconvert -f WAVE -d LEI24@48000 -c 2 "$work/spoken.aiff" "$work/spoken.wav"
heard=0
for level in voice:1.0 quiet:0.01; do
  name=${level%%:*}
  python3 "$here/scripts/session.py" "$work/spoken.wav" "$work/$name" "${level#*:}"
  "$bin/wholeheard-transcription" "$work/$name" --engine speechanalyzer --language fr 2> "$work/$name.err" || true
  segments=$(grep -c '"type":"segment"' "$work/$name/transcript.jsonl" 2>/dev/null || true)
  echo "$name: ${segments:-0} segments"; tail -3 "$work/$name.err"
  [ "$name" = voice ] && heard=${segments:-0}
done
[ "$heard" -gt 0 ] || { echo "::error::a session at a voice's level was heard as nothing"; exit 1; }
