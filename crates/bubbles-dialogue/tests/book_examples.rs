//! Integration tests for dialogue snippets from the mdBook getting-started chapters.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bubbles::{DialogueEvent, HashMapStorage, Runner, RunnerBuilder, Value, compile};
use common::{drain, drain_line_texts, next_options};

const FIRST_DIALOGUE: &str = r"
title: Start
---
Narrator: Hi. I'm the narrator.
===
";

const EVENT_LOOP_ADVENTURE: &str = r"
title: Adventure
---
Narrator: Where do you go?
-> The forest.
    Narrator: You hear wolves.
-> The mountain.
    Narrator: The view is worth it.
===
";

#[test]
fn getting_started_first_dialogue_compiles_and_runs() {
    let prog = compile(FIRST_DIALOGUE).unwrap();
    let mut runner = Runner::new(prog, HashMapStorage::new());
    runner.start("Start").unwrap();
    let events = drain(&mut runner);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DialogueEvent::Line { text, .. } if text.contains("narrator")))
    );
}

#[test]
fn getting_started_event_loop_compiles() {
    let prog = compile(EVENT_LOOP_ADVENTURE).unwrap();
    let mut runner = Runner::new(prog, HashMapStorage::new());
    runner.start("Adventure").unwrap();
    let mut saw_options = false;
    while let Some(ev) = runner.next_event().unwrap() {
        if let DialogueEvent::Options(opts) = ev {
            saw_options = true;
            let idx = opts
                .iter()
                .position(|o| o.available)
                .expect("available option");
            runner.select_option(idx).unwrap();
        }
    }
    assert!(saw_options);
}

#[test]
fn options_snippet_demonstrates_once_options() {
    let source = include_str!("../../../examples/snippets/options.bub");
    let prog = compile(source).unwrap();
    let mut runner = Runner::new(prog, HashMapStorage::new());

    runner.start("Start").unwrap();
    let first_options = next_options(&mut runner);
    let rules_idx = first_options
        .iter()
        .position(|o| o.text.contains("ground rules"))
        .expect("options snippet should include a one-shot rules option");
    assert!(first_options[rules_idx].available);
    runner.select_option(rules_idx).unwrap();
    drain(&mut runner);

    runner.start("Start").unwrap();
    let second_options = next_options(&mut runner);
    assert!(
        !second_options[rules_idx].available,
        "the rules option should be unavailable after it has been selected"
    );
}

/// The `why.md` chapter claims that registering a function under a built-in's
/// name replaces it, and that doing so makes the event stream reproducible.
/// This is the snippet from that chapter, asserted.
///
/// Deliberately feature-agnostic: it must hold whether or not `rand` is
/// compiled in, because replacing the RNG is the recommended route to
/// *reproducible* randomness rather than no randomness.
#[test]
fn why_chapter_seeded_function_override_is_deterministic() {
    const SRC: &str = "title: Roll\n---\n<<declare $n = 0>>\n<<set $n = random_range(1, 6)>>\nNarrator: rolled {$n}.\n<<set $n = random_range(1, 6)>>\nNarrator: rolled {$n}.\n===\n";

    fn play_once() -> Vec<String> {
        let program = compile(SRC).unwrap();

        // A fixed, replayable sequence standing in for the built-in RNG.
        let rolls = Arc::new(vec![3.0, 1.0, 4.0, 1.0, 5.0]);
        let cursor = Arc::new(AtomicUsize::new(0));

        let mut runner = RunnerBuilder::new(program, HashMapStorage::new())
            .with_function("random_range", move |_args| {
                let i = cursor.fetch_add(1, Ordering::Relaxed) % rolls.len();
                Ok(Value::Number(rolls[i]))
            })
            .build();
        runner.start("Roll").unwrap();
        drain_line_texts(&mut runner)
    }

    let first = play_once();
    assert_eq!(
        first,
        vec!["rolled 3.", "rolled 1."],
        "the registered function should replace the built-in `random_range`"
    );

    for _ in 0..8 {
        assert_eq!(
            play_once(),
            first,
            "same program, same variables, same choices must give the same events"
        );
    }
}
