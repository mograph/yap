use crate::library;
use crate::polish;
use crate::store::{self, DictEntry, Dictation, Profile, Rule};
use crate::stt;

#[test]
fn offers_a_list_when_you_run_through_things() {
    assert_eq!(
        polish::as_list("Today I fixed the login bug, shipped the icons, and merged everything."),
        "- Today I fixed the login bug\n- Shipped the icons\n- Merged everything"
    );
    assert_eq!(polish::as_list("Groceries: milk, eggs, and bread"), "Groceries:\n- Milk\n- Eggs\n- Bread");
    assert_eq!(
        polish::as_list("First, call Sam. Then send the deck. Also book the room."),
        "- Call Sam\n- Send the deck\n- Book the room"
    );
    // ordinary sentences stay sentences
    assert_eq!(polish::as_list("I think, honestly, we should just go."), "");
    assert_eq!(polish::as_list("Meet at 10:30, bring snacks"), "");
    assert_eq!(polish::as_list("Can you send me the deck by Wednesday?"), "");
}

#[test]
fn talking_in_commas_is_not_a_list() {
    // People pause constantly. Commas on their own are no evidence of a list at all.
    let not_a_list = [
        "Well, right now, every list is breaking up, it's individual words, rather than, like, actually grouping or having any logic around it.",
        "I went to the store, and it was closed, so I came home, which was annoying",
        "So, anyway, I was thinking, maybe we hold off, at least until Friday, if that works",
        "Yeah, I mean, it's fine, it's just, it's not what I expected, honestly",
    ];
    for said in not_a_list {
        assert_eq!(polish::as_list(said), "", "should not be a list: {said}");
    }
}

#[test]
fn a_spoken_lead_in_is_a_lead_in_not_a_bullet() {
    // Saying you're about to make a list shouldn't put the announcement in the list.
    assert_eq!(
        polish::as_list("Okay so I'm gonna make a list here, I wanna do these tasks, um, fix the login bug, add the settings page, and then write some tests"),
        "Okay so I'm gonna make a list here, I wanna do these tasks:\n- Fix the login bug\n- Add the settings page\n- Write some tests"
    );
    assert_eq!(
        polish::as_list("I'm giving feedback on these, the copy is too long, the button is the wrong colour, and the spacing feels cramped"),
        "I'm giving feedback on these:\n- The copy is too long\n- The button is the wrong colour\n- The spacing feels cramped"
    );
    // an item that happens to mention notes is still an item
    assert_eq!(
        polish::as_list("Give Sam the meeting notes, book the room, and order lunch"),
        "- Give Sam the meeting notes\n- Book the room\n- Order lunch"
    );
}

#[test]
fn one_bullet_per_thing_not_per_comma() {
    // The tail of a thought belongs to the thought, not to a bullet of its own.
    assert_eq!(
        polish::as_list("I fixed the login bug, which kept logging people out, shipped the icons, and merged everything"),
        "- I fixed the login bug, which kept logging people out\n- Shipped the icons\n- Merged everything"
    );
    // three scraps around a lead-in still aren't a list
    assert_eq!(polish::as_list("So I'm gonna make a list, um, yeah, okay"), "");
}

#[test]
fn an_item_that_took_a_few_sentences_stays_one_item() {
    // Going item by item: the cue starts the item, the rest of what you said about it goes with it.
    assert_eq!(
        polish::as_list(
            "Okay I'm gonna make a list, I'm giving feedback on the design. First thing, the header feels too heavy. It's competing with the logo. Second, the sidebar spacing is too tight. And another thing, the button is too orange. I'd tone it down a bit."
        ),
        concat!(
            "Okay I'm gonna make a list, I'm giving feedback on the design:\n",
            "- The header feels too heavy. It's competing with the logo\n",
            "- The sidebar spacing is too tight\n",
            "- The button is too orange. I'd tone it down a bit"
        )
    );
    // No cues at all, but they announced it and took a sentence per thing.
    assert_eq!(
        polish::as_list("I've got a few things. The header feels too heavy. The sidebar spacing is too tight. The button is too orange."),
        concat!(
            "I've got a few things:\n",
            "- The header feels too heavy\n",
            "- The sidebar spacing is too tight\n",
            "- The button is too orange"
        )
    );
    // A plain paragraph is still a paragraph.
    assert_eq!(polish::as_list("The header feels too heavy. It's competing with the logo. I'd tone it down a bit."), "");
}

#[test]
fn bouncing_between_subjects_gathers_them_up() {
    // Two subjects, switched back and forth: each one's sentences end up together, in order.
    assert_eq!(
        polish::as_groups("So the login page is broken, the button doesn't do anything. Also I need to book the flights for next week. The login thing is probably the redirect. Oh and for the flights, get the early one if you can."),
        concat!(
            "So the login page is broken, the button doesn't do anything. The login thing is probably the redirect.\n\n",
            "Also I need to book the flights for next week. Oh and for the flights, get the early one if you can."
        )
    );
    // One subject start to finish: nothing to regroup.
    assert_eq!(polish::as_groups("The login page is broken. The button doesn't do anything when you click it. It's probably the redirect. I'll look at the login code tomorrow."), "");
    // Two subjects, but they already said each one in one go.
    assert_eq!(polish::as_groups("The login page is broken. It's probably the redirect. I need to book the flights. Get the early one if you can."), "");
    // Too short to be worth rearranging.
    assert_eq!(polish::as_groups("The login page is broken. I need to book the flights."), "");
}

#[test]
fn casual_mode_keeps_it_a_message() {
    let mut p = Profile::default();
    p.tone = 95;
    let formal = polish::speaker_section(&p);
    assert!(formal.contains("Polished and clear"));
    assert!(!formal.contains("Casual mode is ON"));

    // Casual caps the register no matter where the slider is, and says so.
    p.casual = true;
    let casual = polish::speaker_section(&p);
    assert!(casual.contains("Casual mode is ON"));
    assert!(casual.contains("friendly Slack message"));
    assert!(!casual.contains("Polished and clear"));
    // The slider value itself is still reported honestly.
    assert!(casual.contains("95 out of 100"));
}

/// Shapes taken from a run over 332 real dictations, where the comma branch was turning
/// ordinary rambling into a bullet per fragment.
#[test]
fn rambling_speech_is_not_chopped_into_fragments() {
    let prose = [
        // Prose across several sentences: commas here are breath, not item boundaries.
        "Fresh or living where there's, like, a random gray bar going down on top of the white, it doesn't make any sense. It needs to be, like, as a principal designer, we need to go through and make sure the animations, as a motion designer, that it makes sense.",
        // One complex sentence that opens on a subordinate clause.
        "Honestly, if we ever switch to Swift apps, we probably won't have the same tokens, or we'll have to convert them.",
        "If I have text that is, like, if I click a box and I'm doing the voice stuff, shouldn't it just auto-paste, or is that something that I need to do?",
        // One question with its conditions tacked on.
        "Hey, I was wondering if this was available, and if the display was working, and if you might be able to go down in price.",
        // Opens on a bare marker and a prepositional phrase.
        "Also, from a copy perspective, the transfer should say, transfer takes seven to ten days, and then there should be a dash, and it should say initiated on blank day.",
        // Items that trail off mid-thought.
        "It needs to be more modern, where it's, like, waiting, like, if you're waiting on someone, or if you're, or it should be more like the other one.",
        "I don't really want, I don't really want it to go to completion, it should be, like, in a state where it's like, oh, there's something that, like, I did, and then I'm supposed to complete later.",
        // A one-word scrap sitting beside full clauses.
        "I'd like you to finish all the changes, then, then publish.",
        "Can you make the state go to the three, not the individual one, and then go to the next part instead of the interspatial step?",
    ];
    for said in prose {
        assert_eq!(polish::as_list(said), "", "should not be a list: {said}");
    }
}

/// The other half of the same run: cues across sentences are the signal that does hold up.
#[test]
fn cues_across_sentences_still_group() {
    assert_eq!(
        polish::as_list(
            "Can you go through this visually? Also, there's so much random motion here. And then there's the settings, but the log is not the same. It's not the pattern of showing each hour."
        ),
        concat!(
            "- Can you go through this visually?\n",
            "- There's so much random motion here\n",
            "- There's the settings, but the log is not the same. It's not the pattern of showing each hour"
        )
    );
}

#[test]
fn the_cloud_only_ever_sees_a_blob_it_cant_read() {
    use crate::cloud;
    let mut profile = Profile::default();
    profile.my_words.push("lowkey".into());
    let history = vec![Dictation { id: "a1".into(), text: "the quarterly numbers are bad".into(), ..Default::default() }];
    let lib = library::Library::new(&profile, &history, &["gone".to_string()]);
    let pass = "seventeen paper lanterns";

    let (salt, nonce, blob) = cloud::seal(pass, &lib).unwrap();
    assert!(!blob.contains("quarterly") && !blob.contains("lowkey"));

    let back = cloud::unseal(pass, &salt, &nonce, &blob).unwrap();
    assert_eq!(back.history[0].text, "the quarterly numbers are bad");
    assert!(back.profile.my_words.iter().any(|w| w == "lowkey"));
    assert_eq!(back.deleted, ["gone"]);

    // A wrong passphrase fails loudly instead of returning junk.
    assert!(cloud::unseal("not the passphrase", &salt, &nonce, &blob).is_err());
    // The same library never uploads as the same bytes twice.
    let (_, _, again) = cloud::seal(pass, &lib).unwrap();
    assert_ne!(blob, again);
}

#[test]
fn both_ways_of_registering_encrypt_the_same() {
    use crate::cloud;
    // Whichever way this computer registered, what reaches Firebase is ciphertext. The only
    // difference is which document it lands in.
    let lib = library::Library::new(&Profile::default(), &[], &[]);
    let pass = "seventeen paper lanterns";
    for anonymous in [true, false] {
        let account = cloud::Account {
            uid: "abc123".into(),
            anonymous,
            passphrase: pass.into(),
            ..Default::default()
        };
        assert!(!account.registered(), "no token yet");
        let (salt, nonce, blob) = cloud::seal(&account.passphrase, &lib).unwrap();
        assert!(cloud::unseal(pass, &salt, &nonce, &blob).is_ok());
        assert!(cloud::unseal("wrong passphrase here", &salt, &nonce, &blob).is_err());
    }
}

#[test]
fn the_vault_id_gives_nothing_away() {
    use crate::cloud;
    let pass = "seventeen paper lanterns";
    let id = cloud::vault_id(pass).unwrap();

    // Same passphrase, same vault: that's how two computers find each other.
    assert_eq!(id, cloud::vault_id(pass).unwrap());
    // A different one lands somewhere else entirely.
    assert_ne!(id, cloud::vault_id("seventeen paper lantern").unwrap());
    // Shaped the way the security rules insist on.
    assert_eq!(id.len(), 32);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));

    // The id must not leak the encryption key: same passphrase, different salt, so the
    // ciphertext can't be derived from the public vault id.
    let lib = library::Library::new(&Profile::default(), &[], &[]);
    let (salt, _, _) = cloud::seal(pass, &lib).unwrap();
    assert!(!id.contains(&salt) && !salt.contains(&id));
}

#[test]
fn a_guessable_passphrase_is_refused() {
    use crate::cloud;
    // Without a sign-in the passphrase is the only thing keeping libraries apart, so the
    // weak ones can't be allowed through.
    for weak in ["", "short", "password", "Password1", "passphrase", "123456789012", "aaaaaaaaaaaaaa", "yap"] {
        assert!(cloud::check_passphrase(weak).is_err(), "should be refused: {weak:?}");
    }
    for good in ["seventeen paper lanterns", "correct horse battery staple", "Tt9!vqm2Zx#4Lp"] {
        assert!(cloud::check_passphrase(good).is_ok(), "should be allowed: {good:?}");
    }
}

#[test]
fn a_spoken_brain_dump_becomes_tasks() {
    // "I want to do a brain dump" and then one thing per sentence: each thing is its own task,
    // and the announcement is the lead-in rather than the first task.
    assert_eq!(
        polish::as_list("Okay I want to do a brain dump. I need to fix the login bug. The settings page needs work. I should email Priya back. And the deploy is still failing."),
        concat!(
            "Okay I want to do a brain dump:\n",
            "- I need to fix the login bug\n",
            "- The settings page needs work\n",
            "- I should email Priya back\n",
            "- The deploy is still failing"
        )
    );
    assert_eq!(
        polish::as_list("Let me do a brain dump real quick. The onboarding copy is too long. We never shipped the empty states. Sam still owes me the icons."),
        concat!(
            "Let me do a brain dump real quick:\n",
            "- The onboarding copy is too long\n",
            "- We never shipped the empty states\n",
            "- Sam still owes me the icons"
        )
    );
    // Said as a run-on instead of separate sentences, it still comes out as tasks.
    assert_eq!(
        polish::as_list("Brain dump: fix the login bug, ship the icons, and email Priya"),
        "Brain dump:\n- Fix the login bug\n- Ship the icons\n- Email Priya"
    );
    // Talking *about* a brain dump isn't asking for one.
    assert_eq!(polish::as_list("The brain dump I sent you yesterday was way too long honestly."), "");
}

#[test]
fn a_dictation_made_during_a_sync_survives_it() {
    let d = |id: &str, at: &str| Dictation { id: id.into(), created_at: at.into(), ..Default::default() };
    let ids = |h: &[Dictation]| {
        let mut v = h.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
        v.sort();
        v
    };

    // The sync starts from a snapshot holding "a", and the cloud holds "b".
    let (mut p, mut h, mut del) = (Profile::default(), vec![d("a", "2026-09-20T10:00:00Z")], Vec::new());
    let cloud = library::Library::new(&Profile::default(), &[d("b", "2026-09-19T10:00:00Z")], &[]);
    let (_, synced) = library::merge(Some(&cloud), &mut p, &mut h, &mut del);

    // While it was on the network, "c" was dictated here. Folding the result in keeps it.
    let (mut now_p, mut now_h, mut now_del) =
        (Profile::default(), vec![d("c", "2026-09-21T10:00:00Z"), d("a", "2026-09-20T10:00:00Z")], Vec::new());
    library::merge(Some(&synced), &mut now_p, &mut now_h, &mut now_del);
    assert_eq!(ids(&now_h), ["a", "b", "c"]);
}

#[test]
fn blank_firebase_settings_are_filled_but_real_ones_kept() {
    let mut blank = store::Settings { firebase_project_id: "".into(), firebase_api_key: "  ".into(), ..Default::default() };
    blank.fill_blank_firebase();
    assert_eq!(blank.firebase_project_id, "yap-tinkerstudio");
    assert!(blank.firebase_api_key.starts_with("AIza"));

    let mut mine = store::Settings { firebase_project_id: "my-own".into(), firebase_api_key: "mine".into(), ..Default::default() };
    mine.fill_blank_firebase();
    assert_eq!((mine.firebase_project_id.as_str(), mine.firebase_api_key.as_str()), ("my-own", "mine"));
}

#[test]
fn library_import_adds_and_never_removes() {
    let mut mine = Profile::default();
    let mut theirs = Profile::default();
    theirs.my_words = vec!["lowkey".into(), "Gonna".into()];
    theirs.dictionary.push(DictEntry { say: "figma".into(), write: "Figma".into() });
    assert_eq!(library::add_profile(&mut mine, &theirs), 2);
    assert!(mine.my_words.iter().any(|w| w == "gonna") && mine.my_words.iter().any(|w| w == "lowkey"));
    assert!(mine.dictionary.iter().any(|e| e.write == "Figma"));
    assert!(mine.dictionary.iter().any(|e| e.write == "Tinker Studio"));
}

#[test]
fn library_folder_syncs_two_computers() {
    let dir = std::env::temp_dir().join(format!("yap-library-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let d = |id: &str, at: &str| Dictation { id: id.into(), created_at: at.into(), text: id.into(), ..Default::default() };
    let ids = |h: &[Dictation]| h.iter().map(|x| x.id.clone()).collect::<Vec<_>>();

    // Computer A goes first.
    let (mut pa, mut ha, mut xa) = (Profile::default(), vec![d("a1", "2026-09-10T10:00:00Z")], Vec::new());
    pa.updated_at = 100;
    assert!(!library::sync(&dir, &mut pa, &mut ha, &mut xa).unwrap());

    // Computer B has a newer profile and its own dictation, and picks up A's.
    let (mut pb, mut hb, mut xb) = (Profile::default(), vec![d("b1", "2026-09-11T10:00:00Z")], Vec::new());
    pb.updated_at = 200;
    pb.my_words.push("lowkey".into());
    assert!(library::sync(&dir, &mut pb, &mut hb, &mut xb).unwrap());
    assert_eq!(ids(&hb), ["b1", "a1"]);

    // Back on A: B's words and dictation arrive.
    assert!(library::sync(&dir, &mut pa, &mut ha, &mut xa).unwrap());
    assert!(pa.my_words.iter().any(|w| w == "lowkey"));
    assert_eq!(ids(&ha), ["b1", "a1"]);

    // A deletes a1, and B doesn't bring it back.
    ha.retain(|x| x.id != "a1");
    xa.push("a1".into());
    library::sync(&dir, &mut pa, &mut ha, &mut xa).unwrap();
    library::sync(&dir, &mut pb, &mut hb, &mut xb).unwrap();
    assert_eq!(ids(&hb), ["b1"]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn kept_pct_ignores_fillers() {
    assert_eq!(store::kept_pct("um so I think", "So I think."), 100.0);
    assert!(store::kept_pct("I am going to the store later", "Heading out soon.") < 30.0);
}

#[test]
fn local_rules_follow_the_profile() {
    let mut profile = Profile::default();
    let out = polish::local_rules(&profile, "um so i'm gonna ping tinker studio uh later");
    assert_eq!(out.text, "So i'm gonna ping Tinker Studio later.");
    assert!(out.edits.iter().any(|e| e.kind == "filler" && e.applied));
    assert!(out.edits.iter().any(|e| e.kind == "dictionary" && e.replacement == "Tinker Studio"));

    profile.rules.insert("filler".into(), Rule::Suggest);
    let out = polish::local_rules(&profile, "um okay");
    assert_eq!(out.text, "Um okay.");
    assert!(out.edits.iter().any(|e| e.kind == "filler" && !e.applied));

    profile.rules.insert("filler".into(), Rule::Leave);
    profile.rules.insert("punctuation".into(), Rule::Leave);
    let out = polish::local_rules(&profile, "um okay");
    assert_eq!(out.text, "um okay");
    assert!(out.edits.is_empty());
}

#[test]
fn local_rules_drop_stutters_repeats_and_long_fillers() {
    let profile = Profile::default();
    let clean = |raw: &str| polish::local_rules(&profile, raw).text;
    assert_eq!(clean("I-I-I think the the plan is uhh fine"), "I think the plan is fine.");
    assert_eq!(clean("so, um, I, I, I wanted to th- the thing"), "So, I wanted to the thing.");
    assert_eq!(clean("Ummm yeah, errr, sounds good"), "Yeah, sounds good.");
    // real words with hyphens stay put
    assert_eq!(clean("I re-read the e-mail"), "I re-read the e-mail.");
    // a word ending one thought and starting the next isn't a stutter (real dictations)
    assert_eq!(
        clean("I don't wanna use the Apple version of it, it sucks"),
        "I don't wanna use the Apple version of it, it sucks."
    );
    assert_eq!(clean("when I tried to click it, it said no"), "When I tried to click it, it said no.");

    let mut keep = Profile::default();
    keep.rules.insert("correction".into(), Rule::Leave);
    assert_eq!(polish::local_rules(&keep, "the the end").text, "The the end.");

    let mut suggest = Profile::default();
    suggest.rules.insert("correction".into(), Rule::Suggest);
    let out = polish::local_rules(&suggest, "I-I-I think");
    assert_eq!(out.text, "I-I-I think.");
    assert!(out.edits.iter().any(|e| e.kind == "correction" && !e.applied));
}

#[test]
fn local_rules_follow_spoken_formatting() {
    let profile = Profile::default();
    let clean = |raw: &str| polish::local_rules(&profile, raw).text;
    assert_eq!(
        clean("okay so today bullet point fixed the login bug, bullet point shipped the icons. Bullet point merged everything"),
        "Okay so today\n- Fixed the login bug\n- Shipped the icons\n- Merged everything"
    );
    assert_eq!(clean("hey new line how are you"), "Hey\nHow are you.");
    assert_eq!(clean("that's it. New paragraph. next up"), "That's it.\n\nNext up.");

    let mut leave = Profile::default();
    leave.rules.insert("formatting".into(), Rule::Leave);
    assert_eq!(polish::local_rules(&leave, "milk bullet point eggs").text, "Milk bullet point eggs.");
}

#[test]
fn tidy_strips_whisper_annotations() {
    assert_eq!(stt::tidy(" [BLANK_AUDIO] hello (music) there "), "hello there");
}

#[test]
fn resamples_48k_to_16k() {
    let input: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.01).sin()).collect();
    assert_eq!(crate::audio::resample(&input, 48_000).len(), 16_000);
}

#[test]
fn splits_long_audio_at_quiet_moments() {
    // 60 s of "speech" with a silent gap at 20 s
    let mut samples = vec![0.3f32; 60 * 16_000];
    samples[20 * 16_000..20 * 16_000 + 3_200].fill(0.0);
    let pieces = stt::split(&samples, 25.0);
    assert!(pieces.iter().all(|p| p.len() <= 25 * 16_000));
    assert_eq!(pieces.iter().map(|p| p.len()).sum::<usize>(), samples.len());
    assert!((pieces[0].len() as i64 - 20 * 16_000).abs() < 3_200, "first cut should land in the gap");
    assert_eq!(stt::split(&samples[..16_000], 25.0).len(), 1);
}

#[test]
fn unshouts_all_caps_but_leaves_normal_text() {
    assert_eq!(stt::unshout("I'M GONNA BE LATE".into()), "i'm gonna be late");
    assert_eq!(stt::unshout("我明天要跟 TINKER STUDIO开会".into()), "我明天要跟 tinker studio开会");
    assert_eq!(stt::unshout("Tell NASA hi".into()), "Tell NASA hi");
    assert_eq!(stt::unshout("嗯那个我明天".into()), "嗯那个我明天");
}

#[test]
fn catalog_ids_are_unique() {
    let mut ids: Vec<_> = stt::CATALOG.iter().map(|s| s.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), stt::CATALOG.len());
}

fn read_wav(path: &str) -> Vec<f32> {
    let wav = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let data = wav.windows(4).position(|w| w == b"data").expect("no data chunk") + 8;
    wav[data..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect()
}

/// Runs every downloaded model over some 16 kHz mono WAV clips and prints speed + transcript.
/// YAP_TEST_DATA=<folder containing models/> YAP_TEST_WAVS=a.wav,b.wav cargo test --release compare -- --ignored --nocapture
#[test]
#[ignore]
fn compare_models() {
    let data = std::path::PathBuf::from(std::env::var("YAP_TEST_DATA").expect("set YAP_TEST_DATA"));
    let clips: Vec<(String, Vec<f32>)> = std::env::var("YAP_TEST_WAVS")
        .expect("set YAP_TEST_WAVS")
        .split(',')
        .map(|p| {
            let name = std::path::Path::new(p).file_stem().unwrap().to_string_lossy().into_owned();
            (name, read_wav(p))
        })
        .collect();
    let vocabulary = stt::vocabulary(&Profile::default());
    let mut ran = 0;
    for spec in stt::CATALOG {
        if !stt::location(&data, spec).exists() {
            continue;
        }
        ran += 1;
        let local = stt::Local::default();
        let started = std::time::Instant::now();
        local.preload(&data, spec.id, &vocabulary);
        println!("\n## {} (load {:.2}s)", spec.label, started.elapsed().as_secs_f32());
        for (name, samples) in &clips {
            let started = std::time::Instant::now();
            let text = local
                .transcribe(&data, spec.id, samples, "auto", &vocabulary)
                .unwrap_or_else(|e| format!("ERROR: {e}"));
            println!(
                "  [{name}: {:.1}s audio -> {:.2}s] {text}",
                samples.len() as f32 / 16_000.0,
                started.elapsed().as_secs_f32()
            );
        }
    }
    assert!(ran > 0, "no models found under {}/models", data.display());
}

/// Feeds sentences through the Mac's cleanup and list logic and writes what came out, so
/// `scripts/ios-parity.sh` can hold the iPhone's Swift port to the same answers.
/// YAP_PARITY_IN=in.json YAP_PARITY_OUT=out.json cargo test --lib parity_dump -- --ignored
#[test]
#[ignore]
fn parity_dump() {
    let input = std::env::var("YAP_PARITY_IN").expect("set YAP_PARITY_IN");
    let output = std::env::var("YAP_PARITY_OUT").expect("set YAP_PARITY_OUT");
    let said: Vec<String> = serde_json::from_str(&std::fs::read_to_string(input).unwrap()).unwrap();
    let profile = Profile::default();
    let rows: Vec<serde_json::Value> = said
        .iter()
        .map(|s| {
            let clean = polish::local_rules(&profile, s).text;
            serde_json::json!({
                "input": s,
                "clean": clean,
                "list": polish::as_list(&clean),
                "groups": polish::as_groups(&clean),
                "listDirect": polish::as_list(s),
                "groupsDirect": polish::as_groups(s),
                "words": store::word_count(&clean),
                "keptPct": format!("{:.2}", store::kept_pct(s, &clean)),
            })
        })
        .collect();
    std::fs::write(output, serde_json::to_string_pretty(&rows).unwrap()).unwrap();

    // What Claude is told about the speaker, for a plain profile and two busy ones.
    if let Ok(prompts) = std::env::var("YAP_PARITY_PROMPTS") {
        let profiles: Vec<Profile> = serde_json::from_str(&std::fs::read_to_string(&prompts).unwrap()).unwrap();
        let out: Vec<String> = profiles.iter().map(polish::speaker_section).collect();
        std::fs::write(format!("{prompts}.rust"), serde_json::to_string_pretty(&out).unwrap()).unwrap();
    }
}

/// The crypto half of `scripts/ios-parity.sh`: key derivation, passphrase rules, and sealing,
/// so the iPhone's Swift can prove it opens what the Mac seals and the other way round.
/// YAP_CRYPTO_MODE=derive|seal|unseal YAP_PARITY_IN=… YAP_PARITY_OUT=… cargo test --lib parity_crypto -- --ignored
#[test]
#[ignore]
fn parity_crypto() {
    use crate::cloud;
    let input = std::fs::read_to_string(std::env::var("YAP_PARITY_IN").unwrap()).unwrap();
    let output = std::env::var("YAP_PARITY_OUT").unwrap();
    let out = match std::env::var("YAP_CRYPTO_MODE").unwrap().as_str() {
        "derive" => {
            let v: serde_json::Value = serde_json::from_str(&input).unwrap();
            let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
            let keys: Vec<serde_json::Value> = v["keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|k| {
                    let (pass, salt) = (k["pass"].as_str().unwrap(), k["salt"].as_str().unwrap());
                    let mut short = [0u8; 16];
                    let mut long = [0u8; 32];
                    argon2::Argon2::default().hash_password_into(pass.as_bytes(), salt.as_bytes(), &mut short).unwrap();
                    argon2::Argon2::default().hash_password_into(pass.as_bytes(), salt.as_bytes(), &mut long).unwrap();
                    serde_json::json!({ "pass": pass, "salt": salt, "k16": hex(&short), "k32": hex(&long), "vault": cloud::vault_id(pass).unwrap() })
                })
                .collect();
            let checks: Vec<serde_json::Value> = v["passphrases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let p = p.as_str().unwrap();
                    serde_json::json!({ "pass": p, "verdict": cloud::check_passphrase(p).err().unwrap_or_default() })
                })
                .collect();
            serde_json::json!({ "keys": keys, "checks": checks })
        }
        "seal" => {
            let v: serde_json::Value = serde_json::from_str(&input).unwrap();
            let lib: library::Library = serde_json::from_value(v["library"].clone()).unwrap();
            let (salt, nonce, blob) = cloud::seal(v["passphrase"].as_str().unwrap(), &lib).unwrap();
            serde_json::json!({ "salt": salt, "nonce": nonce, "blob": blob })
        }
        _ => {
            let v: serde_json::Value = serde_json::from_str(&input).unwrap();
            let s = &v["sealed"];
            let lib = cloud::unseal(v["passphrase"].as_str().unwrap(), s["salt"].as_str().unwrap(),
                                    s["nonce"].as_str().unwrap(), s["blob"].as_str().unwrap()).unwrap();
            serde_json::to_value(&lib).unwrap()
        }
    };
    std::fs::write(output, serde_json::to_string_pretty(&out).unwrap()).unwrap();
}
