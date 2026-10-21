use super::*;
use lambdadx_automation::{
    agent::{
        ChatRequest, Emit,
        provider::{Completion, Provider, Turn},
    },
    contract::ToolError,
};
use std::time::{Duration, Instant};

pub(super) struct TestDir(std::path::PathBuf);
impl TestDir {
    pub(super) fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lambdadx-agent-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    pub(super) fn file(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct FinishedProvider;
impl Provider for FinishedProvider {
    fn complete<'a>(&'a mut self, _: ChatRequest, _: &'a Emit<'a>) -> Completion<'a> {
        Box::pin(async {
            Ok(Turn {
                message: Message::assistant("done"),
                usage: None,
            })
        })
    }
}

#[test]
fn unavailable_review_releases_session_and_allows_a_new_send() {
    let mut host = Service::new();
    let runtime = host.background().unwrap();
    let mut session = Session::default();
    session.run = Some(Run::start(
        &runtime,
        FinishedProvider,
        host.client(),
        vec![],
        lambdadx_automation::contract::Permissions::READ_ONLY,
    ));
    let start = Instant::now();
    loop {
        match session.run.as_ref().unwrap().finished.try_recv() {
            Ok(_) => break,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                assert!(start.elapsed() < Duration::from_secs(2));
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("{error}"),
        }
    }
    // Simulate returning to a pane after its terminal proposal record expired.
    session
        .run
        .as_ref()
        .unwrap()
        .events
        .try_iter()
        .for_each(drop);
    session.state = State::AwaitingApproval;
    session.proposal = Some("expired-record".into());
    session.tick(&host, 1);
    assert_eq!(session.state, State::Failed);
    assert!(!session.busy());
    assert!(session.run.is_none());
    assert!(session.proposal.is_none());
    assert!(session.error.is_some());
    assert!(
        session
            .messages
            .iter()
            .any(|message| format!("{message:?}").contains("editor_review_error"))
    );
}

#[test]
fn review_result_is_recorded_without_starting_another_run() {
    let mut session = Session::default();
    session.state = State::AwaitingApproval;
    let target = lambdadx_automation::contract::SnapshotRef {
        document_id: "doc".into(),
        snapshot_token: "applied-snapshot".into(),
    };
    session.reviewed(ProposalResult {
        proposal_id: "review".into(),
        target: target.clone(),
        state: ProposalState::Applied,
        summary: "move".into(),
        changes: vec![],
        warnings: vec![],
        error: None::<ToolError>,
        applied_target: Some(target),
    });
    assert_eq!(session.state, State::Completed);
    assert!(session.run.is_none());
    let transcript = format!("{:?}", session.messages);
    assert!(transcript.contains("editor_review"));
    assert!(transcript.contains("applied-snapshot"));
}

#[test]
fn restart_restores_transcript_and_enabled_without_reviving_model_or_proposals() {
    let dir = TestDir::new();
    let path = dir.file("history.json");
    let store = history::Store::at(path.clone());
    store
        .save(
            &[
                ("用户".into(), "帮我检查第五小节".into()),
                (
                    "工具 chart_get_context · call-1".into(),
                    "old-snapshot".into(),
                ),
                ("错误".into(), "DOCUMENT_CHANGED".into()),
            ],
            "助手未完成的回复",
            true,
        )
        .unwrap();
    let mut session = Session::restore(
        settings::Saved {
            connection: Settings::default(),
            enabled: true,
        },
        store,
    );
    assert!(session.enabled);
    assert_eq!(session.state, State::Idle);
    assert!(session.run.is_none());
    assert!(session.proposal.is_none());
    assert!(session.messages.is_empty());
    assert!(session.stream.is_empty());
    assert_eq!(session.log[0].1, "帮我检查第五小节");
    assert_eq!(session.log[1].1, "old-snapshot");
    assert_eq!(session.log[2].1, "DOCUMENT_CHANGED");
    assert_eq!(session.log[3].1, "助手未完成的回复");
    assert!(session.log.last().unwrap().1.contains("待审提案未恢复"));
    session.history_saved_at -= Duration::from_secs(1);
    session.tick(&Service::new(), 77);
    assert!(session.messages.is_empty());
    let saved = history::Store::at(path).load().unwrap();
    assert!(!saved.interrupted);
    assert!(saved.stream.is_empty());
}

#[test]
fn disabling_stops_work_and_preserves_partial_reply_but_clear_erases_saved_history() {
    let dir = TestDir::new();
    let path = dir.file("history.json");
    let mut session =
        Session::restore(settings::Saved::default(), history::Store::at(path.clone()));
    session.state = State::Streaming;
    session.record("用户", "检查谱面");
    session.stream = "已经检查到第二小节".into();
    session.messages.push(Message::user("检查谱面"));
    session.proposal = Some("pending".into());
    session.set_enabled(false);
    assert!(!session.enabled);
    assert_eq!(session.state, State::Cancelled);
    assert!(session.proposal.is_none());
    let saved = history::Store::at(path.clone()).load().unwrap();
    assert!(saved.log.iter().any(|(_, t)| t == "已经检查到第二小节"));
    assert!(!saved.interrupted);
    assert!(saved.stream.is_empty());
    session.clear();
    drop(session);
    let restored = Session::restore(settings::Saved::default(), history::Store::at(path));
    assert!(restored.log.is_empty());
    assert!(restored.messages.is_empty());
}

#[test]
fn switching_documents_preserves_records_and_discards_old_model_context() {
    let dir = TestDir::new();
    let path = dir.file("history.json");
    let mut session =
        Session::restore(settings::Saved::default(), history::Store::at(path.clone()));
    session.revision = Some(1);
    session.record("用户", "之前的任务");
    session.messages.push(Message::user("old chart IDs"));
    session.tick(&Service::new(), 2);
    assert!(session.messages.is_empty());
    let saved = history::Store::at(path).load().unwrap();
    assert_eq!(saved.log[0].1, "之前的任务");
    assert!(saved.log.last().unwrap().1.contains("文档已切换"));
}

#[test]
fn history_keeps_newest_entries_within_count_and_byte_limits() {
    let dir = TestDir::new();
    let path = dir.file("history.json");
    let mut session =
        Session::restore(settings::Saved::default(), history::Store::at(path.clone()));
    for i in 0..200 {
        session.record("用户", format!("消息 {i}"));
    }
    assert_eq!(session.log.len(), 128);
    assert_eq!(session.log[0].1, "消息 72");
    for _ in 0..6 {
        session.record("助手", "文".repeat(20_000));
    }
    session.save_history(true);
    let saved = history::Store::at(path).load().unwrap();
    assert_eq!(saved.log.len(), 4);
    assert!(
        saved
            .log
            .iter()
            .map(|(a, b)| a.len() + b.len())
            .sum::<usize>()
            <= MAX_CONTEXT_BYTES
    );
}

#[test]
fn corrupt_history_is_reported_without_preventing_startup_or_overwriting_it() {
    let dir = TestDir::new();
    let path = dir.file("history.json");
    std::fs::write(&path, "broken JSON").unwrap();
    let session = Session::restore(settings::Saved::default(), history::Store::at(path.clone()));
    assert!(session.history_error.is_some());
    assert_eq!(session.state, State::Idle);
    drop(session);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "broken JSON");
}

#[test]
fn history_write_failure_is_visible_and_keeps_unsaved_records() {
    let dir = TestDir::new();
    let parent = dir.file("not-a-directory");
    std::fs::write(&parent, "file").unwrap();
    let mut session = Session::restore(
        settings::Saved::default(),
        history::Store::at(parent.join("history.json")),
    );
    session.record("用户", "不会丢失的内存记录");
    session.save_history(true);
    assert!(session.history_error.is_some());
    assert!(session.history_dirty);
    assert_eq!(session.log[0].1, "不会丢失的内存记录");
}

#[test]
fn checkpoint_saves_latest_stream_before_process_exit_without_resuming_it() {
    let dir = TestDir::new();
    let path = dir.file("history.json");
    let mut session =
        Session::restore(settings::Saved::default(), history::Store::at(path.clone()));
    session.record("用户", "查看这一小节");
    session.state = State::Streaming;
    session.stream = "最后一段回复".into();
    session.checkpoint();
    let saved = history::Store::at(path).load().unwrap();
    assert_eq!(saved.log[0].1, "查看这一小节");
    assert_eq!(saved.stream, "最后一段回复");
    assert!(saved.interrupted);
    assert_eq!(session.state, State::Streaming);
}

#[test]
fn completed_run_flushes_streamed_reply_to_disk_in_order() {
    struct StreamingProvider;
    impl Provider for StreamingProvider {
        fn complete<'a>(&'a mut self, _: ChatRequest, emit: &'a Emit<'a>) -> Completion<'a> {
            Box::pin(async move {
                emit(Event::Text("检查".into()))?;
                emit(Event::Text("完成".into()))?;
                Ok(Turn {
                    message: Message::assistant("检查完成"),
                    usage: None,
                })
            })
        }
    }
    let dir = TestDir::new();
    let path = dir.file("history.json");
    let mut host = Service::new();
    let runtime = host.background().unwrap();
    let mut session =
        Session::restore(settings::Saved::default(), history::Store::at(path.clone()));
    session.record("用户", "检查");
    session.messages.push(Message::user("检查"));
    session.state = State::Requesting;
    session.run = Some(Run::start(
        &runtime,
        StreamingProvider,
        host.client(),
        session.messages.clone(),
        lambdadx_automation::contract::Permissions::READ_ONLY,
    ));
    let start = Instant::now();
    while session.run.is_some() {
        assert!(start.elapsed() < Duration::from_secs(2));
        session.tick(&host, 1);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(session.state, State::Completed);
    let saved = history::Store::at(path).load().unwrap();
    assert_eq!(
        saved.log,
        vec![
            ("用户".into(), "检查".into()),
            ("助手".into(), "检查完成".into())
        ]
    );
    assert!(!saved.interrupted);
    assert!(saved.stream.is_empty());
}
