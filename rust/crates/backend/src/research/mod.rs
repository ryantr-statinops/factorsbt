//! Bounded research protocols layered on the existing simulation engine.

pub mod episode;
pub mod evidence;
pub mod fixtures;
pub mod jobs;
pub mod ranking;
pub mod reranking;

#[cfg(test)]
mod tests {
    use super::{
        episode::{
            baseline_actions, fixed_trade_actions, run_episode, run_episode_with_progress,
            EpisodeAction,
        },
        fixtures::{resolve_fixture, ResearchFixtureKind},
        ranking::{candidates_from_point, rank_both, Candidate, RankingError},
        reranking::compare_reranking,
    };
    use rust_decimal::Decimal;

    #[test]
    fn reranking_uses_same_candidates_and_changes_later_selection() {
        let fixture = resolve_fixture("reranking-eval", "1.0.0").unwrap();
        let report = compare_reranking(&fixture);
        let baseline = report.baseline.unwrap();
        let reranker = report.reranker.unwrap();
        assert_eq!(report.fixture_id, "reranking-eval");
        assert!(baseline
            .selections
            .iter()
            .zip(&reranker.selections)
            .any(|(a, b)| a.candidate.candidate_id != b.candidate.candidate_id));
        assert!(baseline.final_nav > Decimal::ZERO);
        assert!(reranker.final_nav > Decimal::ZERO);
        assert!(baseline.total_filled_value > Decimal::ZERO);
    }

    #[test]
    fn candidate_checks_cover_ties_duplicates_availability_and_targets() {
        let fixture = resolve_fixture("reranking-dev", "1.0.0").unwrap();
        let point = &fixture.points[0];
        let mut candidates = candidates_from_point(point);
        candidates[1].raw_score = Decimal::new(5, 1);
        candidates[2].raw_score = Decimal::new(5, 1);
        let selected = rank_both(
            &candidates,
            &point.close_at,
            Decimal::from(100),
            Decimal::ZERO,
            Decimal::ZERO,
        )
        .unwrap()
        .baseline
        .candidate;
        assert_eq!(selected.candidate_id, "spot-a-50");
        let mut duplicate = candidates.clone();
        duplicate[1].candidate_id = duplicate[0].candidate_id.clone();
        assert!(matches!(
            rank_both(
                &duplicate,
                &point.close_at,
                Decimal::from(100),
                Decimal::ZERO,
                Decimal::ZERO
            ),
            Err(RankingError::DuplicateId(_))
        ));
        let mut future = candidates;
        future[0].available_at = "2026-12-01T16:00:00Z".into();
        assert!(matches!(
            rank_both(
                &future,
                &point.close_at,
                Decimal::from(100),
                Decimal::ZERO,
                Decimal::ZERO
            ),
            Err(RankingError::NotYetAvailable(_))
        ));
        let invalid = vec![Candidate {
            candidate_id: "x".into(),
            raw_score: Decimal::ONE,
            target_a: Decimal::ONE,
            target_b: Decimal::ONE,
            available_at: point.close_at.clone(),
        }];
        assert!(matches!(
            rank_both(
                &invalid,
                &point.close_at,
                Decimal::ONE,
                Decimal::ZERO,
                Decimal::ZERO
            ),
            Err(RankingError::InvalidCandidate(_))
        ));
    }

    #[test]
    fn episode_rewards_include_fees_and_action_sequences_are_isolated() {
        let fixture = resolve_fixture("episode-eval", "1.0.0").unwrap();
        assert_eq!(fixture.kind, ResearchFixtureKind::Episode);
        let baseline = run_episode(&fixture, &baseline_actions()).unwrap();
        assert_eq!(
            baseline
                .transitions
                .iter()
                .map(|step| step.reward_usd)
                .collect::<Vec<_>>(),
            vec![Decimal::ZERO, Decimal::ZERO]
        );
        let actions = fixed_trade_actions();
        let first = run_episode(&fixture, &actions).unwrap();
        let second = run_episode(&fixture, &actions).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.transitions[0].reward_usd, Decimal::from(-1));
        assert_eq!(first.transitions[1].reward_usd, Decimal::from(1469));
        assert_eq!(first.transitions[0].fills[0].fee, Decimal::ONE);
        assert_eq!(first.transitions.len(), 2);
        assert!(run_episode(&fixture, &[]).is_err());
        assert!(run_episode(
            &fixture,
            &[
                EpisodeAction {
                    target_weight: Decimal::new(25, 2)
                },
                EpisodeAction {
                    target_weight: Decimal::ZERO
                }
            ]
        )
        .is_err());
        let full_weight_actions = [
            EpisodeAction {
                target_weight: Decimal::ONE,
            },
            EpisodeAction {
                target_weight: Decimal::ONE,
            },
        ];
        let fully_invested = run_episode(&fixture, &full_weight_actions).unwrap();
        assert!(fully_invested.final_observation.cash >= Decimal::ZERO);
        let mut observed = Vec::new();
        run_episode_with_progress(&fixture, &actions, |transition| {
            observed.push(transition.step)
        })
        .unwrap();
        assert_eq!(observed, vec![0, 1]);
    }
}
