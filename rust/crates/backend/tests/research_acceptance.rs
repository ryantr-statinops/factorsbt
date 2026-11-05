use factorsbt_backend::research::{
    episode::{baseline_actions, fixed_trade_actions, run_episode, run_episode_with_progress},
    fixtures::{resolve_fixture, ResearchFixtureKind},
    ranking::{candidates_from_point, rank_both, Candidate, RankingError},
    reranking::compare_reranking,
};
use rust_decimal::Decimal;

#[test]
fn reranking_uses_the_same_candidates_and_changes_a_later_selection() {
    let fixture = resolve_fixture("reranking-eval", "1.0.0").unwrap();
    let report = compare_reranking(&fixture);
    let baseline = report.baseline.unwrap();
    let reranker = report.reranker.unwrap();
    assert_eq!(report.fixture_id, "reranking-eval");
    assert!(baseline.selections.iter().zip(&reranker.selections).any(|(a, b)| a.candidate.candidate_id != b.candidate.candidate_id));
    assert!(baseline.final_nav > Decimal::ZERO);
    assert!(reranker.final_nav > Decimal::ZERO);
    assert!(baseline.total_filled_value > Decimal::ZERO);
}

#[test]
fn ranking_rejects_duplicate_and_future_candidates_and_breaks_ties_by_id() {
    let fixture = resolve_fixture("reranking-dev", "1.0.0").unwrap();
    let point = &fixture.points[0];
    let mut candidates = candidates_from_point(point);
    candidates[1].raw_score = Decimal::new(5, 1);
    candidates[2].raw_score = Decimal::new(5, 1);
    let selected = rank_both(&candidates, &point.close_at, Decimal::from(100), Decimal::ZERO, Decimal::ZERO).unwrap().baseline.candidate;
    assert_eq!(selected.candidate_id, "spot-a-50");

    let mut duplicate = candidates.clone();
    duplicate[1].candidate_id = duplicate[0].candidate_id.clone();
    assert!(matches!(rank_both(&duplicate, &point.close_at, Decimal::from(100), Decimal::ZERO, Decimal::ZERO), Err(RankingError::DuplicateId(_))));

    let mut future = candidates;
    future[0].available_at = "2026-12-01T16:00:00Z".into();
    assert!(matches!(rank_both(&future, &point.close_at, Decimal::from(100), Decimal::ZERO, Decimal::ZERO), Err(RankingError::NotYetAvailable(_))));
}

#[test]
fn episode_rewards_include_fees_and_progress_only_reports_completed_steps() {
    let fixture = resolve_fixture("episode-eval", "1.0.0").unwrap();
    assert_eq!(fixture.kind, ResearchFixtureKind::Episode);
    let baseline = run_episode(&fixture, &baseline_actions()).unwrap();
    assert_eq!(baseline.transitions.iter().map(|step| step.reward_usd).collect::<Vec<_>>(), vec![Decimal::ZERO, Decimal::ZERO]);

    let actions = fixed_trade_actions();
    let report = run_episode(&fixture, &actions).unwrap();
    assert_eq!(report.transitions[0].reward_usd, Decimal::from(-1));
    assert_eq!(report.transitions[1].reward_usd, Decimal::from(1469));
    assert_eq!(report.transitions[0].fills[0].fee, Decimal::ONE);
    let mut observed = Vec::new();
    run_episode_with_progress(&fixture, &actions, |transition| observed.push(transition.step)).unwrap();
    assert_eq!(observed, vec![0, 1]);
}

#[test]
fn request_level_fixture_versions_are_separate_for_development_and_evaluation() {
    let dev = resolve_fixture("episode-dev", "1.0.0").unwrap();
    let eval = resolve_fixture("episode-eval", "1.0.0").unwrap();
    assert_ne!(dev.points, eval.points);
    assert_ne!(dev.fee_per_fill, eval.fee_per_fill);
    assert!(resolve_fixture("episode-eval", "2.0.0").is_err());
    let invalid = vec![Candidate { candidate_id: "x".into(), raw_score: Decimal::ONE, target_a: Decimal::ONE, target_b: Decimal::ONE, available_at: eval.points[0].close_at.clone() }];
    assert!(matches!(rank_both(&invalid, &eval.points[0].close_at, Decimal::ONE, Decimal::ZERO, Decimal::ZERO), Err(RankingError::InvalidCandidate(_))));
}
