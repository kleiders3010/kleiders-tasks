# UX Principles

*Status: draft, 2026-09-24. Decisions about how the app should feel. Screens and features come later; every rule here should be validated by testing with real people.*

## Decided

1. **Design for the least effort, not the most skill.** The average user has below-average problem-solving patience for a task app, and even skilled users use it distracted, one-handed, in a hurry. If something needs figuring out, it's a design bug, not a user error.
2. **The default experience is the cleanest and simplest to understand.** Nothing optional is visible until someone looks for it.
3. **Choices are something people go looking for.** Options exist (sometimes many), but live one level deeper. They never clutter or complicate the default path.
4. **Default look: like most other task apps. Simple and professional.** Familiar beats novel: people should already know how to use it from other apps.
5. **Alternative looks are equally well designed**, made for other kinds of people, not as afterthoughts. Examples to explore: compact / dense for power users, larger and calmer for simplicity, playful, high contrast.
6. **Appearance is personal, per device.** A look can be shared with a group, where members find it if they go looking. Nobody's choice changes anyone else's app.

## Derived rules (to validate through testing)

These follow from the decisions above and from established usability research (NN/g heuristics). They are not yet tested:

- **No technical words, ever.** Users never see "sync", "relay", "peer", "group key", "P2P", "CRDT", "encryption", etc. in normal use. They see "shared with Alex" and "up to date".
- **Undo instead of confirmation dialogs.** Deleting a task happens immediately, with an Undo option. People don't read "Are you sure?" dialogs anyway.
- **Error messages say what to do**, not what went wrong.
- **Nothing requires instructions.** If a feature needs a tutorial, the design is wrong.
- **State is always visible without being noisy:** whether a list is shared, and whether you're up to date.

## Decided flows

- **"My other device" and "share with people" never appear side by side.**
  - **First launch asks nothing.** The app opens ready to use. Starting fresh and linking later loses nothing: linking merges this device's lists into the user's existing ones.
    - Opened from an invite link → it goes straight to "Join *Groceries*, shared by Uri".
    - Freshly installed from an invite link → the app picks up the invite on first launch (§5 of DESIGN.md) and shows the same join screen.
    - The empty state offers a quiet "Using it on another device? Link this one".
  - Sharing lives on the list ("Share this list"), and devices live under the user's name ("Your devices → Add a device").
  - Scanning or opening any link states what will happen: "Join *Groceries*, shared by Uri" vs "Link this phone to your lists".
  - Linking a device asks for approval on a device the user already has ("Only approve if it's yours").
- **Permissions are always explained first**, on our own screen and in plain words, and asked for only at the moment they become relevant. Examples: exact alarms when a timed reminder is first set or received; local network when a group member is on the same Wi-Fi.

## How we validate

Playtest-style: give ~5 people a task ("add milk to the shared list", "invite me to this list"), watch without helping, fix where they hesitate, repeat.
