# Tasks: shared lists, no account

A free, open-source to-do and shopping list app for Android and Windows. Open it and type: it starts on your last
list. Share a list with a link, QR code or 8 words; changes reach everyone's devices end-to-end encrypted, with no
account and no company server holding your data.

- **Quick:** Enter adds and keeps the field open; "tomorrow at 5" becomes a reminder and "every monday" a repeating one
  (ticked off, it comes back on its next date, like Todoist's); shopping lists bring finished
  items back with one tap; swipe to delete with Undo.
- **Shared:** any list can be shared; everyone edits at once and edits merge (CRDTs, [Loro](https://loro.dev)).
- **No servers you depend on:** devices reach each other directly ([iroh](https://iroh.computer)), over the same
  Wi-Fi, through public Nostr relays, the Mainline DHT, other users' apps that help carry changes, and a small mailbox.
  Every layer only ever sees encrypted data. If one disappears, the others keep working.

## Download
- Android: [Google Play](https://play.google.com/store/apps/details?id=com.jurisgames.tasks) or the APK on the
  [releases page](https://github.com/kleiders3010/jurisgames/releases)
- Windows: installer on the [releases page](https://github.com/kleiders3010/jurisgames/releases)

## Build
- Rust core (`crates/`): `cargo test --workspace`
- Android: `scripts/build-android-libs.sh`, then `cd app && ./gradlew :android:assemblePlayRelease`
- Desktop: `scripts/package-desktop.sh`
- Design: [docs/DESIGN.md](docs/DESIGN.md) · UX: [docs/UX-PRINCIPLES.md](docs/UX-PRINCIPLES.md),
  [docs/UI-SPEC.md](docs/UI-SPEC.md)

## License
Apache-2.0. Icons from Google's Material Icons (Apache-2.0).
