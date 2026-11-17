//! Cancelling agent-owned turns and unowned presentation: an agent-owned turn
//! (`_meta.nori.status`) waits for the agent's `idle`; unowned presentation
//! ends at once.

use super::part10::nori_status_update;
use super::part10::render_bottom_pane;
use super::part10::replay_text_chunk;
use super::*;

fn drain_events(rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>) -> Vec<AppEvent> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

/// Without a live harness, each submitted cancel surfaces as one
/// `HarnessActionFailed`, so counting those counts cancels sent.
fn cancels_sent(events: &[AppEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, AppEvent::HarnessActionFailed(_)))
        .count()
}

fn answer_chunk(message_id: &str, text: &str) -> nori_protocol::SessionEvent {
    replay_text_chunk(crate::presentation::MessageStream::Answer, message_id, text)
}

#[test]
fn agent_owned_turn_cancel_waits_for_idle() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    let generation = chat.session_generation;
    chat.handle_session_event(generation, nori_status_update("working"));
    chat.handle_session_event(generation, answer_chunk("a1", "Working on it."));

    chat.on_ctrl_c();

    assert!(chat.bottom_pane.is_task_running());
    // The composer placeholder is chosen at random, so leave its line out.
    let rendered = render_bottom_pane(&mut chat, 80)
        .lines()
        .filter(|line| !line.starts_with('›'))
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(rendered);

    // Output the agent streams before it stops stays in the same turn.
    chat.handle_session_event(generation, answer_chunk("a2", "Stopping."));
    assert!(chat.proactive_turn_active && chat.proactive_turn_cancelling);

    chat.handle_session_event(generation, nori_status_update("idle"));
    assert!(!chat.bottom_pane.is_task_running());
    assert!(!chat.proactive_turn_active && !chat.proactive_turn_cancelling);

    let events = drain_events(&mut rx);
    assert_eq!(cancels_sent(&events), 1, "{events:#?}");
    assert!(
        !events.iter().any(|e| matches!(e, AppEvent::ExitRequest)),
        "{events:#?}"
    );
}

#[test]
fn repeat_cancel_while_waiting_for_idle_is_ignored() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    let generation = chat.session_generation;
    chat.handle_session_event(generation, nori_status_update("working"));

    // Esc reaches on_cancel_action through AppEvent::HarnessAction(Cancel).
    chat.on_cancel_action();
    chat.on_cancel_action();

    assert!(chat.proactive_turn_cancelling);
    assert!(chat.bottom_pane.is_task_running());
    let events = drain_events(&mut rx);
    assert_eq!(cancels_sent(&events), 1, "{events:#?}");
}

#[test]
fn second_ctrl_c_stops_waiting_for_idle() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    let generation = chat.session_generation;
    chat.handle_session_event(generation, nori_status_update("working"));

    chat.on_ctrl_c();
    chat.on_ctrl_c();

    assert!(!chat.bottom_pane.is_task_running());
    assert!(!chat.proactive_turn_active && !chat.proactive_turn_cancelling);
    let events = drain_events(&mut rx);
    assert_eq!(cancels_sent(&events), 1, "{events:#?}");
    assert!(
        !events.iter().any(|e| matches!(e, AppEvent::ExitRequest)),
        "{events:#?}"
    );
}

#[test]
fn unowned_presentation_cancel_ends_immediately() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    let generation = chat.session_generation;
    chat.handle_session_event(generation, answer_chunk("a1", "Unprompted update."));
    assert!(chat.bottom_pane.is_task_running());

    chat.on_ctrl_c();

    assert!(!chat.bottom_pane.is_task_running());
    assert!(!chat.proactive_turn_active && !chat.proactive_turn_cancelling);
    let events = drain_events(&mut rx);
    assert_eq!(cancels_sent(&events), 1, "{events:#?}");
}
