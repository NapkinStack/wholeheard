#!/usr/bin/env bash
# On a Mac: a session as capture writes it, heard by SpeechAnalyzer through the step, as the app
# hears it — at a voice's level and a hundred times quieter (#426: a Mac's session heard as
# nothing). Fails when the session at a voice's level gives no segment. Run from the repository.
set -euo pipefail
bin=modules/transcription/target/release
here=modules/transcription/speech
cp "$here/.build/release/wholeheard-speech" "$bin/"
# French where the runner hears it, else English: the level and the step are what is tried, not
# the language. Apple's model fetched where the runner offers it and has it not.
locale=
for wanted in fr-FR:fr:fr_FR en-US:en:en_US; do
  code=${wanted%%:*}; status=$("$bin/wholeheard-speech" status "$code" 2>/dev/null || true)
  echo "$code: $status"
  if grep -q '"available":true' <<< "$status" && grep -q '"supported":true' <<< "$status"; then
    grep -q '"installed":true' <<< "$status" || "$bin/wholeheard-speech" install "$code" || continue
    locale=$wanted; break
  fi
done
[ -n "$locale" ] || { echo "::warning::not run — this runner's SpeechTranscriber hears neither fr-FR nor en-US"; exit 0; }
language=$(cut -d: -f2 <<< "$locale"); accent=$(cut -d: -f3 <<< "$locale")
voice=$(say -v '?' | awk -v accent="$accent" '$0 ~ accent {print $1; exit}')
[ -n "$voice" ] || { echo "::warning::not run — no voice for $accent"; exit 0; }
if [ "$language" = fr ]; then
  text="Bonjour à tous, merci d'être là. La réunion commence maintenant. Nous allons parler du budget \
de l'année prochaine, puis du recrutement de deux personnes dans l'équipe technique. Ensuite Karim \
présentera le calendrier du projet Atlas. Enfin nous ferons le point sur les actions de la semaine."
else
  text="Hello everyone, thank you for being here. The meeting starts now. We will talk about next \
year's budget, then about hiring two people for the technical team. Then Karim will present the \
timeline of project Atlas. Finally we will review last week's actions."
fi
work=$(mktemp -d)
say -v "$voice" -o "$work/spoken.aiff" "$text $text $text"
afconvert -f WAVE -d LEI24@48000 -c 2 "$work/spoken.aiff" "$work/spoken.wav"
heard=0
for level in voice:1.0 quiet:0.01; do
  name=${level%%:*}
  python3 "$here/scripts/session.py" "$work/spoken.wav" "$work/$name" "${level#*:}"
  "$bin/wholeheard-transcription" "$work/$name" --engine speechanalyzer --language "$language" 2> "$work/$name.err" || true
  segments=$(grep -c '"type":"segment"' "$work/$name/transcript.jsonl" 2>/dev/null || true)
  echo "$name: ${segments:-0} segments"; tail -3 "$work/$name.err"
  if [ "$name" = voice ]; then heard=${segments:-0}; fi
done
[ "$heard" -gt 0 ] || { echo "::error::a session at a voice's level was heard as nothing"; exit 1; }
