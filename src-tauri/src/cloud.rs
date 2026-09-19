//! Cloud sync, two ways, your pick:
//!
//! - **Just a passphrase.** Yap registers this computer anonymously with Firebase — no
//!   sign-in, no email, no password. Your passphrase picks the vault (`vaults/{id}`) and
//!   unlocks it. Same passphrase on another computer finds the same vault.
//! - **Sign in with Google.** Your account picks the document (`users/{uid}/library/main`)
//!   and Firebase enforces that nobody else can reach it. Sign-in happens in your real
//!   browser, so your Google password goes to Google and never through Yap.
//!
//! Either way the library is encrypted here before it leaves, so Firebase stores a blob it
//! can't read. The difference is what happens if the passphrase is weak: signed in, Firebase
//! still keeps everyone apart; on a passphrase alone, that passphrase is the only thing doing
//! it. `check_passphrase` is what makes the second case safe, and it runs in both.
//!
//! The passphrase never leaves this machine. It sits in `account.json` beside the refresh
//! token, owner-only, so syncing can run in the background. That protects your dictations from
//! Google and from anyone who reaches the database — not from someone using your unlocked Mac.

use crate::library::{self, Library};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write as _IoWrite};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// How this computer is registered with Firebase, and the passphrase everything is encrypted
/// with. Never exported, never uploaded.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Account {
    pub uid: String,
    /// Empty when this computer registered anonymously.
    pub email: String,
    pub refresh_token: String,
    /// True when nobody signed in: the passphrase decides which vault is yours.
    pub anonymous: bool,
    /// Encrypts the library, and on the anonymous path also picks the vault. Never uploaded.
    pub passphrase: String,
}

impl Account {
    pub fn registered(&self) -> bool {
        !self.refresh_token.is_empty()
    }
}

// ---------- the passphrase ----------

/// Shortest passphrase allowed. On the anonymous path it's the only thing keeping libraries
/// apart, so it has to be long enough that the vault id can't be found with a wordlist.
pub const MIN_PASSPHRASE: usize = 12;

/// The passwords every cracking list opens with, plus the ones this app invites.
const OBVIOUS: &[&str] = &[
    "password", "passphrase", "123456", "12345678", "123456789", "qwerty", "letmein", "welcome",
    "admin", "iloveyou", "monkey", "dragon", "sunshine", "princess", "football", "baseball",
    "abc123", "111111", "000000", "changeme", "secret", "yap", "yapyap", "dictation", "chloe",
];

/// Refuses a passphrase that can't do the job. Length first, because it's what actually buys
/// resistance here; then the shapes people reach for when asked to invent one.
pub fn check_passphrase(passphrase: &str) -> Result<(), String> {
    let trimmed = passphrase.trim();
    if trimmed.chars().count() < MIN_PASSPHRASE {
        return Err(format!(
            "That passphrase is too short. It needs at least {MIN_PASSPHRASE} characters — it's what your library is encrypted with."
        ));
    }
    let folded = trimmed.to_lowercase();
    if OBVIOUS.iter().any(|weak| folded == *weak || folded.trim_end_matches(|c: char| c.is_ascii_digit()) == *weak) {
        return Err("That passphrase is one of the first things anyone would try. Pick something else.".into());
    }
    // "aaaaaaaaaaaa" is twelve characters and no harder to guess than one.
    if folded.chars().collect::<std::collections::HashSet<_>>().len() < 5 {
        return Err("That passphrase repeats too few characters to be hard to guess. Pick something else.".into());
    }
    // A long PIN is still a PIN: digits alone are a tiny space to search, whatever the length.
    if folded.chars().all(|c| c.is_ascii_digit()) {
        return Err("That passphrase is only numbers, which is quick to guess. Add words or letters.".into());
    }
    // "abcdefghijkl", "123456789012" and their reverses read as varied but are one guess each.
    let runs = folded
        .as_bytes()
        .windows(2)
        .filter(|w| w[1] as i16 - w[0] as i16 == 1 || w[0] as i16 - w[1] as i16 == 1)
        .count();
    if runs + 1 >= folded.chars().count() {
        return Err("That passphrase is a straight run of characters. Pick something less predictable.".into());
    }
    Ok(())
}

/// Argon2id with a fixed, public salt. Names the vault on the anonymous path; never used to
/// encrypt. The encryption key comes from a *random* salt stored beside the ciphertext, so the
/// vault id gives nothing away about it. Slow on purpose, so vault ids can't be churned through.
pub fn vault_id(passphrase: &str) -> Result<String, String> {
    let mut out = [0u8; 16];
    argon2::Argon2::default()
        .hash_password_into(passphrase.as_bytes(), b"yap-vault-id-v1", &mut out)
        .map_err(|e| format!("Couldn't work out the vault: {e}"))?;
    Ok(out.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    }))
}

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

fn encryption_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Couldn't derive a key: {e}"))?;
    Ok(key)
}

/// Encrypts a library. Fresh salt and nonce every time, so the same library never uploads as
/// the same bytes twice.
pub fn seal(passphrase: &str, lib: &Library) -> Result<(String, String, String), String> {
    let plain = serde_json::to_vec(lib).map_err(|e| e.to_string())?;
    let (mut salt, mut nonce) = ([0u8; SALT_LEN], [0u8; NONCE_LEN]);
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce);
    let key = encryption_key(passphrase, &salt)?;
    let blob = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key))
        .encrypt(Nonce::from_slice(&nonce), plain.as_ref())
        .map_err(|_| "Couldn't encrypt your library.".to_string())?;
    Ok((B64.encode(salt), B64.encode(nonce), B64.encode(blob)))
}

/// Decrypts a library. A wrong passphrase fails here rather than returning nonsense.
pub fn unseal(passphrase: &str, salt: &str, nonce: &str, blob: &str) -> Result<Library, String> {
    let bad = |what: &str| format!("The library in the cloud has a damaged {what}.");
    let salt = B64.decode(salt).map_err(|_| bad("salt"))?;
    let nonce = B64.decode(nonce).map_err(|_| bad("nonce"))?;
    let blob = B64.decode(blob).map_err(|_| bad("payload"))?;
    if nonce.len() != NONCE_LEN {
        return Err(bad("nonce"));
    }
    let key = encryption_key(passphrase, &salt)?;
    let plain = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key))
        .decrypt(Nonce::from_slice(&nonce), blob.as_ref())
        .map_err(|_| "That passphrase doesn't open the library in the cloud.".to_string())?;
    serde_json::from_slice(&plain).map_err(|_| "The library in the cloud isn't readable.".to_string())
}

// ---------- shared HTTP bits ----------

fn enc(s: &str) -> String {
    s.bytes()
        .flat_map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => vec![b as char],
            _ => format!("%{b:02X}").chars().collect(),
        })
        .collect()
}

fn dec(s: &str) -> String {
    let bytes = s.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&String::from_utf8_lossy(&bytes[i + 1..i + 3]), 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect::<Vec<_>>().join("&")
}

async fn post(url: &str, body: String, form_encoded: bool) -> Result<Value, String> {
    let kind = if form_encoded { "application/x-www-form-urlencoded" } else { "application/json" };
    let res = reqwest::Client::new()
        .post(url)
        .header("Content-Type", kind)
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Google: {e}"))?;
    let status = res.status();
    let value: Value = res.json().await.map_err(|e| format!("Google sent something unreadable: {e}"))?;
    if !status.is_success() {
        let message = value["error"]["message"].as_str().unwrap_or("unknown error");
        // The setup steps people miss, named plainly rather than left as API codes.
        if message.contains("ADMIN_ONLY_OPERATION") {
            return Err("Turn on Anonymous sign-in in the Firebase console, then try again.".into());
        }
        if message.contains("OPERATION_NOT_ALLOWED") {
            return Err("Turn on the Google sign-in provider in the Firebase console, then try again.".into());
        }
        return Err(format!("Google refused: {message}"));
    }
    Ok(value)
}

// ---------- registering without an account ----------

/// Registers this computer with Firebase without anyone signing in. Firebase hands back an
/// anonymous user so its security rules have something to check; it isn't an account, has no
/// email or password, and it isn't what decides which vault you see.
pub async fn register_anonymously(api_key: &str) -> Result<Account, String> {
    let value = post(
        &format!("https://identitytoolkit.googleapis.com/v1/accounts:signUp?key={}", enc(api_key)),
        json!({ "returnSecureToken": true }).to_string(),
        false,
    )
    .await?;
    Ok(Account {
        uid: value["localId"].as_str().unwrap_or_default().to_string(),
        email: String::new(),
        refresh_token: value["refreshToken"].as_str().unwrap_or_default().to_string(),
        anonymous: true,
        passphrase: String::new(),
    })
}

// ---------- signing in with Google ----------

/// Google's sign-in page for this app, plus what's needed to prove we started it.
pub struct Start {
    pub url: String,
    pub verifier: String,
    pub state: String,
    pub listener: TcpListener,
    pub redirect: String,
}

fn random_b64(bytes: usize) -> String {
    let mut raw = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut raw);
    B64.encode(raw)
}

/// Opens a port for Google to redirect back to and builds the sign-in URL. A loopback redirect
/// with PKCE is Google's flow for installed apps: the client secret isn't what's trusted, the
/// proof is that we hold the verifier.
pub fn start_sign_in(client_id: &str) -> Result<Start, String> {
    if client_id.trim().is_empty() {
        return Err("Add your Google client ID in Settings first.".into());
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("Couldn't open a port to sign in: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{port}");
    let verifier = random_b64(48);
    let challenge = B64.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_b64(16);
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?{}",
        form(&[
            ("client_id", client_id),
            ("redirect_uri", &redirect),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("state", &state),
            ("prompt", "select_account"),
        ])
    );
    Ok(Start { url, verifier, state, listener, redirect })
}

/// Waits for Google to send the browser back here. Blocking: run it off the UI thread.
pub fn wait_for_code(start: &Start, timeout: Duration) -> Result<String, String> {
    start.listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    let mut stream = loop {
        match start.listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err("Sign-in timed out.".into());
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => return Err(format!("Sign-in failed: {e}")),
        }
    };
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;

    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).map_err(|e| e.to_string())?;
    let query = line
        .split_whitespace()
        .nth(1)
        .and_then(|target| target.split_once('?').map(|(_, q)| q.to_string()))
        .unwrap_or_default();
    let (mut code, mut state, mut error) = (String::new(), String::new(), String::new());
    for pair in query.split('&') {
        match pair.split_once('=') {
            Some(("code", v)) => code = dec(v),
            Some(("state", v)) => state = dec(v),
            Some(("error", v)) => error = dec(v),
            _ => {}
        }
    }

    let ok = error.is_empty() && !code.is_empty() && state == start.state;
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>Yap</title>\
         <body style=\"font:15px -apple-system,system-ui,sans-serif;display:grid;place-items:center;height:100vh;margin:0;background:#f6f4ef;color:#1a1712\">\
         <p>{}</p>",
        if ok { "Signed in. You can close this tab and go back to Yap." } else { "Sign-in didn't finish. You can close this tab." }
    );
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();

    if !error.is_empty() {
        return Err(format!("Google said: {error}"));
    }
    if code.is_empty() {
        return Err("Sign-in was cancelled.".into());
    }
    // A mismatched state means this redirect wasn't the one we started.
    if state != start.state {
        return Err("Sign-in didn't match the request that started it.".into());
    }
    Ok(code)
}

/// Trades the code for a Google ID token, then trades that for a Firebase session.
pub async fn finish_sign_in(
    api_key: &str,
    client_id: &str,
    client_secret: &str,
    redirect: &str,
    verifier: &str,
    code: &str,
) -> Result<Account, String> {
    let token = post(
        "https://oauth2.googleapis.com/token",
        form(&[
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code_verifier", verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect),
        ]),
        true,
    )
    .await?;
    let google_id_token = token["id_token"].as_str().unwrap_or_default();
    if google_id_token.is_empty() {
        return Err("Google didn't return an ID token.".into());
    }

    let session = post(
        &format!("https://identitytoolkit.googleapis.com/v1/accounts:signInWithIdp?key={}", enc(api_key)),
        json!({
            "postBody": format!("id_token={google_id_token}&providerId=google.com"),
            "requestUri": redirect,
            "returnSecureToken": true,
        })
        .to_string(),
        false,
    )
    .await?;
    Ok(Account {
        uid: session["localId"].as_str().unwrap_or_default().to_string(),
        email: session["email"].as_str().unwrap_or_default().to_string(),
        refresh_token: session["refreshToken"].as_str().unwrap_or_default().to_string(),
        anonymous: false,
        passphrase: String::new(),
    })
}

/// A short-lived token for Firestore, from the long-lived refresh token.
async fn id_token(api_key: &str, refresh_token: &str) -> Result<String, String> {
    let value = post(
        &format!("https://securetoken.googleapis.com/v1/token?key={}", enc(api_key)),
        form(&[("grant_type", "refresh_token"), ("refresh_token", refresh_token)]),
        true,
    )
    .await?;
    match value["id_token"].as_str() {
        Some(token) if !token.is_empty() => Ok(token.to_string()),
        _ => Err("This computer's Firebase session expired. Sign in again, or turn cloud sync off and on.".into()),
    }
}

// ---------- Firestore ----------

/// Where this account's library lives. Signed in, Firebase enforces the `{uid}` in the path;
/// anonymous, the passphrase is what names the vault.
fn doc_path(account: &Account) -> Result<String, String> {
    if account.anonymous {
        Ok(format!("vaults/{}", vault_id(&account.passphrase)?))
    } else {
        Ok(format!("users/{}/library/main", account.uid))
    }
}

fn doc_url(project: &str, path: &str) -> String {
    format!("https://firestore.googleapis.com/v1/projects/{project}/databases/(default)/documents/{path}")
}

async fn fetch(project: &str, path: &str, token: &str) -> Result<Option<Value>, String> {
    let res = reqwest::Client::new()
        .get(doc_url(project, path))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Firestore: {e}"))?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None); // nothing synced from here yet
    }
    let status = res.status();
    let value: Value = res.json().await.map_err(|e| format!("Firestore sent something unreadable: {e}"))?;
    if !status.is_success() {
        return Err(format!("Firestore refused: {}", value["error"]["message"].as_str().unwrap_or("unknown error")));
    }
    Ok(Some(value))
}

async fn store(project: &str, path: &str, token: &str, salt: &str, nonce: &str, blob: &str) -> Result<(), String> {
    let body = json!({
        "fields": {
            "app": { "stringValue": "yap" },
            "v": { "integerValue": "1" },
            "salt": { "stringValue": salt },
            "nonce": { "stringValue": nonce },
            "blob": { "stringValue": blob },
            "updatedAt": { "integerValue": chrono::Utc::now().timestamp_millis().to_string() },
        }
    });
    let res = reqwest::Client::new()
        .patch(doc_url(project, path))
        .bearer_auth(token)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Firestore: {e}"))?;
    if res.status().is_success() {
        return Ok(());
    }
    let value: Value = res.json().await.unwrap_or_default();
    Err(format!("Firestore refused: {}", value["error"]["message"].as_str().unwrap_or("unknown error")))
}

/// What one round of syncing did, for the UI to report.
pub struct Synced {
    pub changed: bool,
    pub uploaded: bool,
}

/// Two-way sync with Firestore: pull, decrypt, merge, push back if this computer knows
/// something it didn't. Same merge rules as the folder sync, so both can run at once.
pub async fn sync(
    project: &str,
    api_key: &str,
    account: &Account,
    profile: &mut crate::store::Profile,
    history: &mut Vec<crate::store::Dictation>,
    deleted: &mut Vec<String>,
) -> Result<Synced, String> {
    if project.trim().is_empty() || api_key.trim().is_empty() {
        return Err("Add your Firebase project and API key in Settings first.".into());
    }
    if account.passphrase.is_empty() {
        return Err("Set a passphrase in Settings. Your library is encrypted with it before it leaves this computer.".into());
    }
    // Also catches a passphrase saved by an older build, before the strength rule existed.
    check_passphrase(&account.passphrase)?;

    let path = doc_path(account)?;
    let token = id_token(api_key, &account.refresh_token).await?;

    let remote = match fetch(project, &path, &token).await? {
        Some(doc) => {
            let field = |name: &str| doc["fields"][name]["stringValue"].as_str().unwrap_or_default();
            let (salt, nonce, blob) = (field("salt"), field("nonce"), field("blob"));
            if salt.is_empty() || nonce.is_empty() || blob.is_empty() {
                None
            } else {
                Some(unseal(&account.passphrase, salt, nonce, blob)?)
            }
        }
        None => None,
    };

    let (changed, merged) = library::merge(remote.as_ref(), profile, history, deleted);
    let uploaded = library::differs(remote.as_ref(), &merged);
    if uploaded {
        let (salt, nonce, blob) = seal(&account.passphrase, &merged)?;
        store(project, &path, &token, &salt, &nonce, &blob).await?;
    }
    Ok(Synced { changed, uploaded })
}
