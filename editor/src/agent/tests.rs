use super::*;
use lambdadx_automation::{
    agent::{
        ChatRequest, Emit,
        provider::{Completion, Provider, Turn},
    },
    contract::ToolError,
};
use std::time::{Duration, Instant};

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
