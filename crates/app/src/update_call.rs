use std::sync::{Arc, Mutex};

use iced::Task;

use crate::message::Message;
use crate::state::FluxChat;

/// The call to leave if the process exits without a close request: on macOS,
/// Cmd+Q and the Quit menu terminate the app directly (winit never sends
/// `CloseRequested`), and the call membership would show us in the call for
/// hours. Set while `call_room_id` is.
static CALL_ON_EXIT: Mutex<Option<(Arc<fluxchat_core::AppState>, String, tokio::runtime::Handle)>> = Mutex::new(None);

/// Most of an error shown in the notice line.
const FAILURE_NOTICE_CHARS: usize = 200;

/// Record (or clear) the call `leave_call_on_exit` leaves.
pub(crate) fn set_call_on_exit(core: &Arc<fluxchat_core::AppState>, room_id: Option<&str>) {
    let entry = room_id.zip(tokio::runtime::Handle::try_current().ok())
        .map(|(rid, rt)| (core.clone(), rid.to_string(), rt));
    *CALL_ON_EXIT.lock().unwrap_or_else(|e| e.into_inner()) = entry;
}

#[cfg(target_os = "macos")]
pub(crate) fn register_exit_hook() {
    extern "C" fn on_exit() {
        leave_call_on_exit();
    }
    // SAFETY: registers a plain function with no captured state.
    unsafe { libc::atexit(on_exit) };
}

/// Leave the recorded call, waiting at most 3 s. Runs from `atexit`, where
/// the runtime's worker threads are still alive; the request runs on its own
/// thread so nothing here can unwind into C.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn leave_call_on_exit() {
    let Some((core, room_id, rt)) = CALL_ON_EXIT.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let _ = std::thread::spawn(move || {
        rt.block_on(async {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), async {
                if let Ok(client) = core.get_client().await {
                    let _ = fluxchat_core::rtc::leave_call(&client, &room_id).await;
                }
            }).await;
        });
    }).join();
}

/// Whether the app has let go of the session a call task belongs to: it was
/// told to disconnect (left, failed), or its handle is gone (a newer call).
fn released(session: &tokio::sync::watch::Receiver<bool>) -> bool {
    *session.borrow() || session.has_changed().is_err()
}

/// The notice for a call that couldn't be joined or broke off. `e` may be a
/// server's whole error page: its first line is enough.
fn failure_notice(e: &str) -> String {
    if e.contains("M_FORBIDDEN") {
        return "Call failed: you're not allowed to join calls in this room".into();
    }
    if e.contains("No LiveKit") || e.contains("rtc_foci") {
        return "Call failed: this server isn't set up for calls".into();
    }
    let line: String = e.lines().next().unwrap_or("").chars().take(FAILURE_NOTICE_CHARS).collect();
    format!("Call failed: {line}")
}

impl FluxChat {
    /// Take our membership out of a room's call: left in the room's state, it
    /// lists us in the call until it expires, hours later.
    fn remove_call_membership(&self, room_id: String) -> Task<Message> {
        let core = self.core.clone();
        Task::perform(
            async move {
                let client = core.get_client().await.ok()?;
                fluxchat_core::rtc::leave_call(&client, &room_id).await.ok()
            },
            |_| Message::Noop,
        )
    }

    /// Let go of the call session, if one is up: its task disconnects, and
    /// nothing it still reports is passed on (see `released`).
    fn release_call_session(&mut self) {
        if let Some(tx) = self.call_disconnect_tx.take() {
            let _ = tx.send(true);
        }
        self.call_mute_tx = None;
    }

    /// Handle all call/RTC messages. Returns Some(task) if handled.
    pub(crate) fn handle_call_message(&mut self, message: Message) -> Option<Task<Message>> {
        match message {
            Message::CallJoin(room_id) => {
                if self.call_room_id.is_some() {
                    return Some(Task::none());
                }
                // The header's button names the view on screen, which may be
                // a thread: the call is its room's.
                let room_id = fluxchat_core::timeline::real_room_id(&room_id).to_string();
                self.call_state = fluxchat_core::rtc::CallConnectionState::Connecting;
                self.call_room_id = Some(room_id.clone());
                // From here the list is the call's own (LiveKit identities).
                // What the room's state said goes by user ID: kept, those
                // would stay behind as doubles that never leave.
                self.call_participants.clear();
                set_call_on_exit(&self.core, Some(&room_id));
                self.messages_at_bottom = true;
                let core = self.core.clone();
                let snap = iced::widget::operation::snap_to_end(crate::view::messages_scrollable_id());
                Some(Task::batch([snap, Task::perform(
                    async move {
                        let client = core.get_client().await.map_err(|_| "Not logged in".to_string())?;
                        fluxchat_core::rtc::join_call(&client, &room_id).await.map_err(|e| e.to_string())
                    },
                    Message::CallJoinResult,
                )]))
            }
            Message::CallJoinResult(result) => {
                match result {
                    Ok(info) => {
                        // Left (or signed out) while the request was out. It has
                        // written our membership by now: take that back, and
                        // don't connect to a call nothing here could hang up.
                        if self.call_room_id.as_deref() != Some(info.room_id.as_str()) {
                            return Some(self.remove_call_membership(info.room_id));
                        }
                        self.calls_supported = true;
                        let msg_tx = self.call_event_tx.clone();
                        let info_clone = info.clone();

                        let (disconnect_tx, disconnect_rx) = tokio::sync::watch::channel(false);
                        self.call_disconnect_tx = Some(disconnect_tx);
                        // Calls start muted; the mic button talks to the session
                        // task through this (it used to flip only the icon).
                        let (mute_tx, mut mute_rx) = tokio::sync::watch::channel(true);
                        self.call_mute_tx = Some(mute_tx);
                        self.call_mic_muted = true;

                        let (call_tx, mut call_rx) = tokio::sync::mpsc::unbounded_channel::<fluxchat_core::rtc_audio::CallEvent>();
                        let msg_tx2 = msg_tx.clone();
                        let session = disconnect_rx.clone();
                        tokio::spawn(async move {
                            while let Some(event) = call_rx.recv().await {
                                // A session the app has let go of still reports
                                // its own disconnect, which would be taken for
                                // the end of the call joined after it.
                                if released(&session) {
                                    break;
                                }
                                use fluxchat_core::rtc_audio::CallEvent;
                                let msg = match event {
                                    CallEvent::Connected => Message::CallConnected,
                                    CallEvent::Disconnected(reason) => Message::CallDisconnected(reason),
                                    CallEvent::ParticipantJoined(p) => Message::CallParticipantJoined(p),
                                    CallEvent::ParticipantLeft(id) => Message::CallParticipantLeft(id),
                                    CallEvent::ActiveSpeakersChanged(_) => Message::Noop,
                                    CallEvent::Failed(e) => Message::CallError(e),
                                };
                                let _ = msg_tx2.send(msg);
                            }
                        });

                        let mut disconnect_rx = disconnect_rx;
                        tokio::spawn(async move {
                            match fluxchat_core::rtc_audio::CallSession::connect(&info_clone, call_tx).await {
                                Ok(mut session) => {
                                    if let Err(e) = session.publish_microphone().await {
                                        tracing::warn!("Failed to publish mic: {e}");
                                        // Ends the call: the app lets go of the
                                        // session, which the loop below sees.
                                        let _ = msg_tx.send(Message::CallError(e));
                                    }
                                    session.set_mic_muted(true);
                                    // The watch `Ref`s aren't Send: keep them out of
                                    // the select's outputs and off every await.
                                    let mut mute_open = true;
                                    loop {
                                        tokio::select! {
                                            _ = async { let _ = disconnect_rx.wait_for(|v| *v).await; } => break,
                                            changed = mute_rx.changed(), if mute_open => match changed {
                                                Ok(()) => {
                                                    let muted = *mute_rx.borrow_and_update();
                                                    session.set_mic_muted(muted);
                                                }
                                                // The app let go of the call: only leaving is left.
                                                Err(_) => mute_open = false,
                                            },
                                        }
                                    }
                                    tracing::info!("Disconnecting LiveKit session");
                                    session.disconnect().await;
                                }
                                Err(e) => {
                                    if !released(&disconnect_rx) {
                                        let _ = msg_tx.send(Message::CallError(e));
                                    }
                                }
                            }
                        });
                        Some(Task::none())
                    }
                    Err(e) => {
                        // Of a call that was left meanwhile.
                        if self.call_room_id.take().is_none() {
                            return Some(Task::none());
                        }
                        // Nothing was joined, so the call is free again: kept,
                        // it made the call button do nothing until the failure
                        // was dismissed in the member list (which may be hidden).
                        set_call_on_exit(&self.core, None);
                        let notice = self.update(Message::ShowError(failure_notice(&e)));
                        self.call_state = fluxchat_core::rtc::CallConnectionState::Failed(e);
                        Some(notice)
                    }
                }
            }
            Message::CallConnected => {
                // From a session that was let go of on the way.
                if self.call_room_id.is_none() {
                    return Some(Task::none());
                }
                self.call_state = fluxchat_core::rtc::CallConnectionState::Connected;
                self.messages_at_bottom = true;
                Some(iced::widget::operation::snap_to_end(crate::view::messages_scrollable_id()))
            }
            Message::CallDisconnected(reason) => {
                tracing::info!("Call disconnected: {reason}");
                self.call_state = fluxchat_core::rtc::CallConnectionState::Disconnected;
                set_call_on_exit(&self.core, None);
                self.call_participants.clear();
                self.release_call_session();
                // Still in a call here means it wasn't hung up from this side
                // (the server ended it, or the connection was lost): our
                // membership is still in the room.
                Some(match self.call_room_id.take() {
                    Some(room_id) => self.remove_call_membership(room_id),
                    None => Task::none(),
                })
            }
            Message::CallError(e) => {
                tracing::warn!("Call error: {e}");
                // From a session that was let go of on the way.
                let Some(room_id) = self.call_room_id.take() else { return Some(Task::none()) };
                // The call is over. Hang up what is still connected (a
                // microphone that can't be opened leaves the session up: heard,
                // with nothing on screen to end it) and take our membership
                // back, which the join wrote.
                set_call_on_exit(&self.core, None);
                self.release_call_session();
                self.call_participants.clear();
                let notice = self.update(Message::ShowError(failure_notice(&e)));
                self.call_state = fluxchat_core::rtc::CallConnectionState::Failed(e);
                Some(Task::batch([self.remove_call_membership(room_id), notice]))
            }
            Message::CallLeave => {
                let Some(room_id) = self.call_room_id.take() else { return Some(Task::none()) };
                self.release_call_session();
                self.call_state = fluxchat_core::rtc::CallConnectionState::Disconnected;
                set_call_on_exit(&self.core, None);
                self.call_participants.clear();
                Some(self.remove_call_membership(room_id))
            }
            Message::CallParticipantJoined(participant) => {
                if self.call_room_id.is_some()
                    && !self.call_participants.iter().any(|p| p.user_id == participant.user_id)
                {
                    self.call_participants.push(participant);
                }
                Some(Task::none())
            }
            Message::CallParticipantLeft(identity) => {
                self.call_participants.retain(|p| p.user_id != identity);
                Some(Task::none())
            }
            Message::CallParticipantsUpdated(room_id, participants) => {
                if !participants.is_empty() {
                    self.rooms_with_calls.insert(room_id.clone());
                } else {
                    self.rooms_with_calls.remove(&room_id);
                }
                // In a call the list is that call's, whichever room is on
                // screen: looking at another room used to empty it.
                if self.call_room_id.is_none() && self.selected_room_id.as_deref() == Some(room_id.as_str()) {
                    self.call_participants = participants;
                }
                Some(Task::none())
            }
            Message::CallToggleMic => {
                self.call_mic_muted = !self.call_mic_muted;
                if let Some(tx) = &self.call_mute_tx {
                    let _ = tx.send(self.call_mic_muted);
                }
                Some(Task::none())
            }
            Message::CallRoomsWithCallsUpdated(rooms) => {
                self.calls_supported = true;
                self.rooms_with_calls = rooms;
                Some(Task::none())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use fluxchat_core::rtc::{CallConnectionState, CallJoinInfo, CallParticipant};

    use super::{failure_notice, released};
    use crate::message::Message;
    use crate::panes::flow_tests::app;

    fn participant(id: &str) -> CallParticipant {
        CallParticipant {
            user_id: id.into(), device_id: String::new(), display_name: id.into(),
            avatar_url: None, is_local: false,
        }
    }

    fn joined(room: &str) -> Message {
        Message::CallJoinResult(Ok(CallJoinInfo {
            room_id: room.into(), livekit_url: "wss://lk.invalid".into(), jwt: "jwt".into(),
        }))
    }

    /// In a connected call in `room`, with the session handles a join leaves.
    fn in_call(room: &str) -> (crate::state::FluxChat, tokio::sync::watch::Receiver<bool>) {
        let mut app = app();
        let _ = app.update(Message::CallJoin(room.into()));
        let (tx, rx) = tokio::sync::watch::channel(false);
        app.call_disconnect_tx = Some(tx);
        let _ = app.update(Message::CallConnected);
        assert_eq!(app.call_state, CallConnectionState::Connected);
        (app, rx)
    }

    #[tokio::test]
    async fn a_failed_join_says_so_and_leaves_the_call_button_working() {
        let mut app = app();
        let _ = app.update(Message::CallJoin("!a:x".into()));
        assert_eq!(app.call_state, CallConnectionState::Connecting);
        let _ = app.update(Message::CallJoinResult(Err("LiveKit JWT request failed (502): <html>\n<body>…".into())));
        assert!(matches!(app.call_state, CallConnectionState::Failed(_)));
        assert_eq!(app.help_text.as_deref(), Some("Call failed: LiveKit JWT request failed (502): <html>"));
        // The next press tries again (it was ignored until the failure was
        // dismissed in the member list).
        let _ = app.update(Message::CallJoin("!a:x".into()));
        assert_eq!(app.call_state, CallConnectionState::Connecting);
        assert_eq!(app.call_room_id.as_deref(), Some("!a:x"));
    }

    #[tokio::test]
    async fn the_call_button_in_a_thread_view_joins_the_rooms_call() {
        let mut app = app();
        let _ = app.update(Message::CallJoin(fluxchat_core::timeline::thread_key("!a:x", "$root")));
        assert_eq!(app.call_room_id.as_deref(), Some("!a:x"));
    }

    #[tokio::test]
    async fn a_join_that_lands_after_leaving_does_not_connect() {
        let mut app = app();
        let _ = app.update(Message::CallJoin("!a:x".into()));
        let _ = app.update(Message::CallLeave);
        // The request went through meanwhile: our membership is in the room.
        let undo = app.update(joined("!a:x"));
        assert!(app.call_disconnect_tx.is_none(), "no session for a call that was left");
        assert_eq!(app.call_state, CallConnectionState::Disconnected);
        assert_eq!(undo.units(), 1, "the membership it wrote is taken back");
        // Nor does a join for another room than the one being joined now.
        let _ = app.update(Message::CallJoin("!b:x".into()));
        let _ = app.update(joined("!a:x"));
        assert!(app.call_disconnect_tx.is_none());
        assert_eq!(app.call_room_id.as_deref(), Some("!b:x"));
    }

    #[tokio::test]
    async fn a_call_ended_from_the_other_side_takes_our_membership_out() {
        let (mut app, session) = in_call("!a:x");
        let leave = app.update(Message::CallDisconnected("ServerShutdown".into()));
        assert_eq!(leave.units(), 1, "still listed in the call otherwise, for hours");
        assert!(app.call_room_id.is_none());
        assert_eq!(app.call_state, CallConnectionState::Disconnected);
        assert!(released(&session));
        // Hung up from here: Leave did that already.
        let (mut app, _session) = in_call("!a:x");
        assert_eq!(app.update(Message::CallLeave).units(), 1);
        assert_eq!(app.update(Message::CallDisconnected("ClientInitiated".into())).units(), 0);
    }

    #[tokio::test]
    async fn a_call_that_breaks_is_hung_up_and_can_be_joined_again() {
        // The microphone couldn't be opened: the session is up and playing.
        let (mut app, session) = in_call("!a:x");
        let _ = app.update(Message::CallParticipantJoined(participant("@b:x:DEV")));
        let end = app.update(Message::CallError("Failed to open PulseAudio capture".into()));
        assert!(released(&session), "nothing would hang it up otherwise");
        assert!(matches!(app.call_state, CallConnectionState::Failed(_)));
        assert!(app.call_room_id.is_none() && app.call_participants.is_empty());
        assert!(end.units() >= 1, "the membership the join wrote is taken back");
        assert_eq!(app.help_text.as_deref(), Some("Call failed: Failed to open PulseAudio capture"));
        // Dismissing the failure has nothing left to leave.
        assert_eq!(app.update(Message::CallDisconnected("dismissed".into())).units(), 0);
        assert_eq!(app.call_state, CallConnectionState::Disconnected);
        let _ = app.update(Message::CallJoin("!a:x".into()));
        assert_eq!(app.call_state, CallConnectionState::Connecting);
    }

    #[tokio::test]
    async fn reports_of_a_session_that_was_let_go_change_nothing() {
        let (mut app, _session) = in_call("!a:x");
        let _ = app.update(Message::CallLeave);
        let _ = app.update(Message::CallConnected);
        let _ = app.update(Message::CallParticipantJoined(participant("@b:x:DEV")));
        let _ = app.update(Message::CallError("LiveKit connect failed".into()));
        assert_eq!(app.call_state, CallConnectionState::Disconnected);
        assert!(app.call_participants.is_empty());
        assert!(app.help_text.is_none());
    }

    #[tokio::test]
    async fn a_session_is_released_once_told_to_disconnect_or_dropped() {
        let (tx, rx) = tokio::sync::watch::channel(false);
        assert!(!released(&rx));
        let _ = tx.send(true);
        assert!(released(&rx));
        let (tx, rx) = tokio::sync::watch::channel(false);
        drop(tx);
        assert!(released(&rx));
    }

    #[tokio::test]
    async fn the_calls_participants_survive_looking_at_another_room() {
        let mut app = app();
        app.selected_room_id = Some("!a:x".into());
        // What the room's state lists before joining (user IDs)…
        let _ = app.update(Message::CallParticipantsUpdated("!a:x".into(), vec![participant("@b:x")]));
        assert_eq!(app.call_participants.len(), 1);
        // …makes way for the call's own list (LiveKit identities).
        let _ = app.update(Message::CallJoin("!a:x".into()));
        let _ = app.update(Message::CallConnected);
        let _ = app.update(Message::CallParticipantJoined(participant("@b:x:DEV")));
        assert_eq!(app.call_participants, vec![participant("@b:x:DEV")], "not listed twice");

        // Another room on screen, with no call of its own.
        app.selected_room_id = Some("!other:x".into());
        let _ = app.update(Message::CallParticipantsUpdated("!other:x".into(), vec![]));
        assert_eq!(app.call_participants.len(), 1, "still the people in our call");
        let _ = app.update(Message::CallParticipantLeft("@b:x:DEV".into()));
        assert!(app.call_participants.is_empty());
    }

    #[test]
    fn failure_notices_are_short_and_say_what_is_known() {
        assert_eq!(failure_notice("M_FORBIDDEN: no"), "Call failed: you're not allowed to join calls in this room");
        assert_eq!(failure_notice("No LiveKit service URL (rtc_foci)"), "Call failed: this server isn't set up for calls");
        let long = "x".repeat(5000);
        assert!(failure_notice(&long).chars().count() < 250);
    }
}
