"""Public exception types for SDK transport and contract failures."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .responses import RunStatus


class FactorsBTError(Exception):
    """Base class for errors raised by the FactorsBT SDK."""


class TransportError(FactorsBTError):
    """The backend could not be reached or returned an unusable response."""


class RequestTimeout(TransportError):
    """The backend did not respond before the configured timeout."""


class HttpStatusError(FactorsBTError):
    """The backend returned an unsuccessful HTTP status."""

    def __init__(
        self,
        status: int,
        *,
        code: str | None = None,
        message: str | None = None,
        request_id: str | None = None,
        details: tuple[tuple[str, str], ...] = (),
    ):
        self.status = status
        self.code = code
        self.message = message
        self.request_id = request_id
        self.details = details
        description = f"backend returned HTTP {status}"
        if code:
            description += f" ({code})"
        super().__init__(description)


class ProtocolError(FactorsBTError):
    """The backend response did not match the health wire contract."""


class SubmissionAcknowledgementUnknown(TransportError):
    """A run POST ended without a trustworthy acknowledgment; look up its key."""

    def __init__(self, submission_id: str, reason: str):
        self.submission_id = submission_id
        self.reason = reason
        super().__init__(
            f"run submission acknowledgment is unknown; look up submission_id {submission_id}"
        )


class RunFailedError(FactorsBTError):
    """A run reached its failed terminal state while the caller was waiting."""

    def __init__(self, status: RunStatus):
        self.status = status
        self.run_id = status.run_id
        self.failure = status.failure
        code = status.failure.code if status.failure is not None else "UNKNOWN_RUN_FAILURE"
        super().__init__(f"run {status.run_id} failed ({code})")


class WaitTimeout(RequestTimeout):
    """The local wait deadline expired; the backend run continues independently."""

    def __init__(self, run_id: str, last_status: RunStatus | None):
        self.run_id = run_id
        self.last_status = last_status
        super().__init__(
            f"local wait deadline expired for run {run_id}; backend work was not cancelled"
        )


class ReplayAcknowledgementUnknown(TransportError):
    def __init__(self, replay_id: str, reason: str):
        self.replay_id = replay_id
        self.reason = reason
        super().__init__(f"replay acknowledgment is unknown; look up replay_id {replay_id}")


class ReplayFailedError(FactorsBTError):
    def __init__(self, status):
        self.status = status
        self.replay_id = status.replay_id
        super().__init__(f"replay {status.replay_id} failed ({status.failure_code or 'UNKNOWN_REPLAY_FAILURE'})")
