"""Run the bounded Phase D research jobs against a local FactorsBT backend."""

from decimal import Decimal
import os

from factorsbt import Client


def main() -> None:
    client = Client(os.environ.get("FACTORSBT_BASE_URL", "http://127.0.0.1:8080"))

    ranking = client.submit_reranking("reranking-eval", "1.0.0")
    client.wait_research_job(ranking.job_id)
    ranking_result = client.get_research_result(ranking.job_id)
    print("reranking outcome vs baseline:", ranking_result.report.outcome_vs_baseline)

    episode = client.submit_episode(
        "episode-eval", "1.0.0", [Decimal("0.5"), Decimal("0.5")]
    )
    client.wait_research_job(episode.job_id)
    episode_result = client.get_research_result(episode.job_id)
    print("episode rewards:", [row.reward_usd for row in episode_result.report.transitions])

    replay = client.submit_research_replay(episode.job_id)
    client.wait_research_job(replay.job_id)
    print("replay matched:", client.get_research_result(replay.job_id).report.matched)


if __name__ == "__main__":
    main()
