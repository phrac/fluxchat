use std::sync::{Arc, Mutex};

use iced::Task;

use crate::message::Message;
use crate::state::FluxChat;

/// The call to leave if the process exits without a close request: on macOS,
/// Cmd+Q and the Quit menu terminate the app directly (winit never sends
/// `CloseRequested`), and the call membership would show us in the call for
/// hours. Set while `call_room_id` is.
static CALL_ON_EXIT: Mutex<Option<(Arc<fluxchat_core::AppState>, String, tokio::runtime::Handle)>> = Mutex::new(None);

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

impl FluxChat {
    /// Handle all call/RTC messages. Returns Some(task) if handled.
    pub(crate) fn handle_call_message(&mut self, message: Message) -> Option<Task<Message>> {
        match message {
            Message::CallJoin(room_id) => {
                if self.call_room_id.is_some() {
                    return Some(Task::none());
                }
                self.call_state = fluxchat_core::rtc::CallConnectionState::Connecting;
                self.call_room_id = Some(room_id.clone());
                set_call_on_exit(&self.core, Some(&room_id));
                self.messages_at_bottom = true;
                let core = self.core.clone();
                let snap = iced::widget::operation::snap_to_end(crate::view::messages_scrollable_id());
                Some(Task::batch([snap, Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        match fluxchat_core::rtc::join_call(&client, &room_id).await {
                            Ok(info) => Some(Ok(info)),
                            Err(e) => Some(Err(format!("{e}"))),
                        }
                    },
                    |result| match result {
                        Some(r) => Message::CallJoinResult(r),
                        None => Message::CallError("Not logged in".into()),
                    },
                )]))
            }
            Message::CallJoinResult(result) => {
                match result {
                    Ok(info) => {
                        self.calls_supported = true;
                        let msg_tx = self.call_event_tx.clone();
                        let info_clone = info.clone();

                        let (disconnect_tx, disconnect_rx) = tokio::sync::watch::channel(false);
                        self.call_disconnect_tx = Some(disconnect_tx);

                        let (call_tx, mut call_rx) = tokio::sync::mpsc::unbounded_channel::<fluxchat_core::rtc_audio::CallEvent>();
                        let msg_tx2 = msg_tx.clone();
                        tokio::spawn(async move {
                            while let Some(event) = call_rx.recv().await {
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
                                        let _ = msg_tx.send(Message::CallError(e));
                                    }
                                    session.set_mic_muted(true);
                                    let _ = disconnect_rx.wait_for(|v| *v).await;
                                    tracing::info!("Disconnecting LiveKit session");
                                    session.disconnect().await;
                                }
                                Err(e) => {
                                    let _ = msg_tx.send(Message::CallError(e));
                                }
                            }
                        });
                    }
                    Err(e) => {
                        self.call_state = fluxchat_core::rtc::CallConnectionState::Failed(e);
                    }
                }
                Some(Task::none())
            }
            Message::CallConnected => {
                self.call_state = fluxchat_core::rtc::CallConnectionState::Connected;
                self.messages_at_bottom = true;
                Some(iced::widget::operation::snap_to_end(crate::view::messages_scrollable_id()))
            }
            Message::CallDisconnected(reason) => {
                tracing::info!("Call disconnected: {reason}");
                self.call_state = fluxchat_core::rtc::CallConnectionState::Disconnected;
                self.call_room_id = None;
                set_call_on_exit(&self.core, None);
                self.call_participants.clear();
                self.call_disconnect_tx = None;
                Some(Task::none())
            }
            Message::CallError(e) => {
                tracing::warn!("Call error: {e}");
                self.call_state = fluxchat_core::rtc::CallConnectionState::Failed(e);
                Some(Task::none())
            }
            Message::CallLeave => {
                if let Some(ref room_id) = self.call_room_id.clone() {
                    if let Some(tx) = self.call_disconnect_tx.take() {
                        let _ = tx.send(true);
                    }
                    let core = self.core.clone();
                    let rid = room_id.clone();
                    self.call_state = fluxchat_core::rtc::CallConnectionState::Disconnected;
                    self.call_room_id = None;
                    set_call_on_exit(&self.core, None);
                    self.call_participants.clear();
                    Some(Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rtc::leave_call(&client, &rid).await.ok()
                        },
                        |_| Message::Noop,
                    ))
                } else {
                    Some(Task::none())
                }
            }
            Message::CallParticipantJoined(participant) => {
                if !self.call_participants.iter().any(|p| p.user_id == participant.user_id) {
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
                let in_active_call = self.call_room_id.as_deref() == Some(room_id.as_str())
                    && self.call_state == fluxchat_core::rtc::CallConnectionState::Connected;
                if !in_active_call && self.selected_room_id.as_deref() == Some(room_id.as_str()) {
                    self.call_participants = participants;
                }
                Some(Task::none())
            }
            Message::CallToggleMic => {
                self.call_mic_muted = !self.call_mic_muted;
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
