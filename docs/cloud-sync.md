# Cloud sync setup

Two ways to set up a computer. You pick, per computer, the first time you turn cloud sync on.

**Just a passphrase.** Yap registers this computer anonymously with Firebase — no account,
nothing to sign in to. Your passphrase picks the vault (`vaults/{id}`, Argon2id of the
passphrase) and unlocks it. Any computer with the same passphrase finds the same library.

**Sign in with Google.** Your account picks the document (`users/{uid}/library/main`) and
Firebase enforces that nobody else can reach it. Sign-in runs in your real browser via a
loopback redirect with PKCE, so your Google password goes to Google and never through Yap.

Either way the library is encrypted here before it leaves, under a key derived from your
passphrase with a random salt — so Firebase stores a blob it can't read.

## Which to choose

Signed in, Firebase keeps accounts apart, so a weak passphrase only risks the encryption.
On a passphrase alone, that passphrase is both the address and the key, so a guessable one is
guessable twice over. That's why `check_passphrase` runs in both modes and isn't optional: at
least 12 characters, not a common password, not all digits, not a straight run like
`123456789012`, not a handful of repeated characters. It runs when you set the passphrase and
again before every sync, so one saved by an older build can't slip through.

The passphrase never leaves the machine. It lives in `account.json` in the app data dir next
to the refresh token, written owner-only (`0600`). That protects your dictations from Google
and from anyone who reaches the database — not from someone using your unlocked Mac.

**There is no recovery.** Lose the passphrase and the cloud copy can't be opened. Everything on
your computer is untouched.

## Project

Created and configured already, on the free Spark plan:

| | |
|---|---|
| Project ID | `yap-tinkerstudio` |
| Web API key | `AIzaSyALock_PLlhBo6sZgQPCPBsoKMQI7b4tko` |
| Console | https://console.firebase.google.com/project/yap-tinkerstudio/overview |

The web API key is not a secret. Firebase API keys identify a project; the security rules are
what guard the data.

**Done:** project created, web app registered, Firestore database created, security rules
deployed.

## Console steps

**For the passphrase-only path** — enable Anonymous:
https://console.firebase.google.com/project/yap-tinkerstudio/authentication/providers →
**Anonymous** → Save

**For Google sign-in** — enable the Google provider on that same page, then make a Desktop
OAuth client:

1. Consent screen, once: https://console.cloud.google.com/auth/overview?project=yap-tinkerstudio
2. https://console.cloud.google.com/apis/credentials?project=yap-tinkerstudio →
   **Create credentials → OAuth client ID → Desktop app**
3. Paste the client ID and secret into Settings → Cloud sync → Firebase details

A Desktop client is the right kind: a Web client rejects the loopback redirect Yap uses.

Turn on whichever you want. If a provider is off, Yap says so in plain words rather than
showing a Firebase error code.

## On the iPhone (Moonshot)

The iPhone app uses the same vaults, the same encryption and the same merge as the Mac, so a
library flows between them either way. Settings → Account, same two choices.

- **Passphrase only** needs nothing extra: the project and API key come pre-filled.
- **Google sign-in** needs an OAuth client of the **iOS** type, not the Desktop one the Mac
  uses: https://console.cloud.google.com/apis/credentials?project=yap-tinkerstudio →
  **Create credentials → OAuth client ID → iOS**, bundle ID `io.tinkerstudio.moonshot.ios`.
  Paste its client ID under Account → Firebase details. No secret is needed.

The account and passphrase live in the iOS Keychain, on that device only (never iCloud Keychain),
readable after first unlock so a sync can finish in the background.

Argon2id isn't in CryptoKit, so `ios/Yap/Argon2.swift` implements it. It has to produce exactly
the Mac's bytes or the phone can never open the Mac's vault, so `scripts/ios-parity.sh` derives
keys on both sides and seals a library on each side for the other to open. Derived keys are
cached per launch: the first sync after opening the app takes a moment, the rest are instant.

## When it syncs

Both apps sync after every dictation, every couple of minutes while open, and when opened (the
phone also when it comes back to the front). **Sync now** always runs, even with the toggle off.
One sync at a time, and a sync folds its result into whatever is there when it finishes, so a
dictation made mid-sync is kept.

## Rules

Deployed. Re-deploy after editing `firestore.rules`:

```sh
firebase deploy --only firestore:rules --project yap-tinkerstudio
```

Reads and writes need an authenticated caller (anonymous counts), the vault id must be 32 hex
characters, and a written document must look like a Yap library and stay under 2 MB. Everything
else in the project is denied.

## Using it

Settings → Cloud sync → on → pick **Use a passphrase** or **Sign in with Google** → set a
passphrase → **Sync now**.

On your other computer: same passphrase. Signed in, use the same Google account too.

**Forget this computer** drops the local registration and passphrase. Nothing local is deleted,
and what's in the cloud stays for whatever still has the passphrase or the account.

Cloud sync and the shared-folder sync use the same merge rules, so running both is safe: newest
profile wins, history and deletions union from both sides.
